use bridge::TyCtxt;
use bridge::hir::def::{DefKind, Res};
use bridge::hir::def_id::DefId;
use bridge::middle::ty::{self, GenericArg, GenericArgKind, Ty, TypeVisitable, TypeVisitor};
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct Variances(HashMap<DefId, Vec<ty::Variance>>);
impl Variances {
    pub fn of_arguments(&mut self, tcx: TyCtxt<'_>, res: Res) -> Vec<ty::Variance> {
        match res {
            Res::Def(DefKind::Struct | DefKind::Enum | DefKind::Union, id) => {
                tcx.variances_of(id).to_vec()
            }
            Res::Def(DefKind::TyAlias, id) => self
                .0
                .entry(id)
                .or_insert_with(|| alias_variances(tcx, id))
                .clone(),
            _ => Vec::new(),
        }
    }
}

fn alias_variances(tcx: TyCtxt<'_>, alias: DefId) -> Vec<ty::Variance> {
    let mut uses = ParameterUses {
        tcx,
        variances: vec![ty::Bivariant; tcx.generics_of(alias).count()],
    };
    uses.visit(tcx.type_of(alias).skip_binder(), ty::Covariant);
    uses.variances
}

struct ParameterUses<'tcx> {
    tcx: TyCtxt<'tcx>,
    variances: Vec<ty::Variance>,
}
impl<'tcx> ParameterUses<'tcx> {
    fn record(&mut self, index: u32, variance: ty::Variance) {
        if let Some(known) = self.variances.get_mut(index as usize) {
            *known = match (*known, variance) {
                (ty::Bivariant, other) | (other, ty::Bivariant) => other,
                (known, other) if known == other => known,
                _ => ty::Invariant,
            };
        }
    }

    fn region(&mut self, region: ty::Region<'tcx>, variance: ty::Variance) {
        if let ty::ReEarlyParam(param) = region.kind() {
            self.record(param.index, variance);
        }
    }

    fn argument(&mut self, argument: GenericArg<'tcx>, variance: ty::Variance) {
        match argument.kind() {
            GenericArgKind::Lifetime(region) => self.region(region, variance),
            GenericArgKind::Type(ty) => self.visit(ty, variance),
            GenericArgKind::Const(_) => {}
        }
    }

    fn visit(&mut self, ty: Ty<'tcx>, variance: ty::Variance) {
        match *ty.kind() {
            ty::Param(param) => self.record(param.index, variance),
            ty::Ref(region, inner, mutability) => {
                self.region(region, variance);
                self.visit(inner, pointee(variance, mutability));
            }
            ty::RawPtr(inner, mutability) => self.visit(inner, pointee(variance, mutability)),
            ty::Adt(def, arguments) => {
                let variances = self.tcx.variances_of(def.did());
                for (argument, declared) in arguments.iter().zip(variances) {
                    self.argument(argument, variance.xform(*declared));
                }
            }
            ty::Tuple(elements) => {
                for element in elements {
                    self.visit(element, variance);
                }
            }
            ty::Array(element, _) | ty::Slice(element) => self.visit(element, variance),
            ty::FnPtr(signature, _) => {
                let signature = signature.skip_binder();
                for input in signature.inputs() {
                    self.visit(*input, variance.xform(ty::Contravariant));
                }
                self.visit(signature.output(), variance);
            }
            ty::Dynamic(predicates, region) => {
                self.region(region, variance);
                predicates.visit_with(&mut InvariantUses(self));
            }
            _ => ty.visit_with(&mut InvariantUses(self)),
        }
    }
}

fn pointee(variance: ty::Variance, mutability: ty::Mutability) -> ty::Variance {
    match mutability {
        ty::Mutability::Not => variance,
        ty::Mutability::Mut => ty::Invariant,
    }
}

struct InvariantUses<'a, 'tcx>(&'a mut ParameterUses<'tcx>);
impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for InvariantUses<'_, 'tcx> {
    fn visit_ty(&mut self, ty: Ty<'tcx>) {
        match *ty.kind() {
            ty::Param(param) => self.0.record(param.index, ty::Invariant),
            _ => ty::TypeSuperVisitable::super_visit_with(&ty, self),
        }
    }
    fn visit_region(&mut self, region: ty::Region<'tcx>) {
        self.0.region(region, ty::Invariant);
    }
}
