use super::variance::Variances;
use crate::TargetFunc;
use bridge::hir::{
    self,
    def_id::{DefId, LocalDefId},
    intravisit::{self, Visitor},
};
use bridge::middle::middle::resolve_bound_vars::ResolvedArg;
use bridge::middle::traits::query::OutlivesBound;
use bridge::middle::ty;
use bridge::span::{SourceFile, Span};
use bridge::{FromRustc, TyCtxt};
use models::{
    LifetimeAnnotation, LifetimeRelation, Polarity, Signature, SignaturePosition, SkipReason, Slot,
    SlotSyntax,
};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct Extracted {
    pub signature: Signature,
    pub guarded: Vec<(usize, SkipReason)>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Anonymous {
    Rejected,
    Numbered,
}

fn lifetimes(
    generics: &hir::Generics<'_>,
    anonymous: Anonymous,
    scope: &str,
) -> Vec<(LocalDefId, LifetimeAnnotation)> {
    generics
        .params
        .iter()
        .filter(|param| matches!(param.kind, hir::GenericParamKind::Lifetime { .. }))
        .enumerate()
        .filter_map(|(index, param)| {
            let name = LifetimeAnnotation::from(param.name.ident().name.as_str());
            match (name.is_anonymous(), anonymous) {
                (false, _) => Some((param.def_id, name)),
                (true, Anonymous::Numbered) => Some((
                    param.def_id,
                    LifetimeAnnotation::from(format!("_{scope}#{index}")),
                )),
                (true, Anonymous::Rejected) => None,
            }
        })
        .collect()
}

pub fn named_lifetimes(generics: &hir::Generics<'_>) -> Vec<(LocalDefId, LifetimeAnnotation)> {
    lifetimes(generics, Anonymous::Rejected, "")
}

fn written_relations(generics: &hir::Generics<'_>) -> Vec<LifetimeRelation> {
    generics
        .predicates
        .iter()
        .filter_map(|predicate| match predicate.kind {
            hir::WherePredicateKind::RegionPredicate(region) => Some(region),
            _ => None,
        })
        .flat_map(|region| {
            region.bounds.iter().filter_map(|bound| match bound {
                hir::GenericBound::Outlives(shorter) => Some(LifetimeRelation {
                    longer: LifetimeAnnotation::from(region.lifetime.ident.name.as_str()),
                    shorter: LifetimeAnnotation::from(shorter.ident.name.as_str()),
                }),
                _ => None,
            })
        })
        .collect()
}

pub fn extract<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: &TargetFunc<'tcx>,
    variances: Variances,
    anonymous: Anonymous,
) -> Result<Extracted, SkipReason> {
    if target
        .generics
        .predicates
        .iter()
        .any(|predicate| !matches!(predicate.kind, hir::WherePredicateKind::RegionPredicate(_)))
    {
        return Err(SkipReason::BoundPredicate);
    }
    let params = lifetimes(target.generics, anonymous, "fn");
    let fixed = target
        .parent_generics
        .map(|generics| lifetimes(generics, anonymous, "impl"))
        .unwrap_or_default();
    let own: HashMap<_, _> = params.iter().chain(&fixed).cloned().collect();
    let decl = target.signature.decl;
    let mut collector = SlotCollector {
        tcx,
        variances,
        source: target.source.clone(),
        own: own.clone(),
        slots: Vec::new(),
        guarded: Vec::new(),
        object_defaults: Vec::new(),
        position: SignaturePosition::Return,
        polarity: Polarity::Pos,
        binders: 0,
        receiver: None,
        failure: None,
    };
    for (index, input) in decl.inputs.iter().enumerate() {
        collector.position = SignaturePosition::Params(index);
        collector.polarity = Polarity::Neg;
        collector.receiver = match (index, decl.implicit_self(), input.kind) {
            (
                0,
                hir::ImplicitSelfKind::RefImm | hir::ImplicitSelfKind::RefMut,
                hir::TyKind::Ref(lifetime, ..),
            ) => Some(lifetime.ident.span),
            _ => None,
        };
        collector.visit_ty_unambig(input);
    }
    collector.position = SignaturePosition::Return;
    collector.polarity = Polarity::Pos;
    collector.receiver = None;
    collector.visit_fn_ret_ty(&decl.output);
    let SlotCollector {
        slots,
        guarded,
        object_defaults,
        failure,
        ..
    } = collector;
    if let Some(reason) = failure {
        return Err(reason);
    }

    let mut signature = Signature {
        late_bound: target
            .generics
            .params
            .iter()
            .filter(|param| tcx.is_late_bound(param.hir_id))
            .filter_map(|param| own.get(&param.def_id).cloned())
            .collect(),
        params: params.into_iter().map(|(_, name)| name).collect(),
        fixed: fixed.into_iter().map(|(_, name)| name).collect(),
        slots,
        predicates: written_relations(target.generics),
        known: target
            .parent_generics
            .map(written_relations)
            .unwrap_or_default(),
    };
    for (lifetime, polarity) in object_defaults {
        if signature.polarity(&lifetime) != polarity
            && let Some(slot) = signature
                .slots
                .iter_mut()
                .find(|slot| slot.lifetime == lifetime)
        {
            slot.polarity = Polarity::Inv;
        }
    }
    let universe = signature.universe();
    signature.known.extend(
        implied_relations(tcx, target.def_id, &own)?
            .into_iter()
            .filter(|relation| {
                universe.contains(&relation.longer) && universe.contains(&relation.shorter)
            }),
    );
    Ok(Extracted { signature, guarded })
}

fn implied_relations(
    tcx: TyCtxt<'_>,
    def_id: LocalDefId,
    own: &HashMap<LocalDefId, LifetimeAnnotation>,
) -> Result<Vec<LifetimeRelation>, SkipReason> {
    let generics = tcx.generics_of(def_id);
    let declared = |id: DefId| id.as_local().and_then(|id| own.get(&id)).cloned();
    let name = |region: ty::Region<'_>| match region.kind() {
        ty::ReStatic => Some(LifetimeAnnotation::static_lifetime()),
        ty::ReEarlyParam(param) => declared(generics.region_param(param, tcx).def_id),
        ty::ReLateParam(param) => match param.kind {
            ty::LateParamRegionKind::Named(id) => declared(id),
            _ => None,
        },
        _ => None,
    };
    let bounds = tcx
        .mir_borrowck_implied_outlives_bounds(def_id)
        .map_err(|_| SkipReason::WellFormednessErrors)?;
    Ok(bounds
        .value
        .value
        .outlives_bounds
        .iter()
        .filter_map(|bound| match *bound {
            OutlivesBound::RegionSubRegion(shorter, longer) => Some(LifetimeRelation {
                longer: name(longer)?,
                shorter: name(shorter)?,
            }),
            _ => None,
        })
        .collect())
}

struct SlotCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    variances: Variances,
    source: Arc<SourceFile>,
    own: HashMap<LocalDefId, LifetimeAnnotation>,
    slots: Vec<Slot>,
    guarded: Vec<(usize, SkipReason)>,
    object_defaults: Vec<(LifetimeAnnotation, Polarity)>,
    position: SignaturePosition,
    polarity: Polarity,
    binders: usize,
    receiver: Option<Span>,
    failure: Option<SkipReason>,
}
impl SlotCollector<'_> {
    fn under(&self, variance: ty::Variance) -> Polarity {
        match variance {
            ty::Covariant => self.polarity,
            ty::Contravariant => self.polarity.reverse(),
            ty::Invariant | ty::Bivariant => Polarity::Inv,
        }
    }

    fn object_default(&mut self, lifetime: &hir::Lifetime) {
        let declared = match self.tcx.named_bound_var(lifetime.hir_id) {
            Some(ResolvedArg::EarlyBound(id) | ResolvedArg::LateBound(_, _, id)) => Some(id),
            _ => None,
        };
        if let Some(name) = declared.and_then(|id| self.own.get(&id)) {
            self.object_defaults.push((name.clone(), self.polarity));
        }
    }
}
impl<'hir> Visitor<'hir> for SlotCollector<'hir> {
    fn visit_ty(&mut self, ty: &'hir hir::Ty<'hir, hir::AmbigArg>) -> Self::Result {
        let outer = self.polarity;
        match ty.kind {
            hir::TyKind::Ref(lifetime, inner, mutability) => {
                self.visit_lifetime(lifetime);
                if mutability.is_mut() {
                    self.polarity = Polarity::Inv;
                }
                self.visit_ty_unambig(inner);
            }
            hir::TyKind::Ptr(inner, mutability) => {
                if mutability.is_mut() {
                    self.polarity = Polarity::Inv;
                }
                self.visit_ty_unambig(inner);
            }
            hir::TyKind::FnPtr(pointer) => {
                self.binders += 1;
                self.polarity = outer.reverse();
                for input in pointer.decl.inputs {
                    self.visit_ty_unambig(input);
                }
                self.polarity = outer;
                self.visit_fn_ret_ty(&pointer.decl.output);
                self.binders -= 1;
            }
            hir::TyKind::TraitObject(bounds, lifetime) => {
                let index = self.slots.len();
                self.visit_lifetime(lifetime.pointer());
                if self.slots.len() != index {
                    self.guarded
                        .push((index, SkipReason::ObjectLifetimeDefault));
                }
                self.polarity = Polarity::Inv;
                for bound in bounds {
                    self.visit_poly_trait_ref(bound);
                }
            }
            hir::TyKind::OpaqueDef(_) => {
                self.failure.get_or_insert(SkipReason::ImplTraitReturn);
            }
            hir::TyKind::UnsafeBinder(_) => {
                self.failure.get_or_insert(SkipReason::HigherRanked);
            }
            hir::TyKind::Path(hir::QPath::Resolved(Some(_), _) | hir::QPath::TypeRelative(..)) => {
                self.polarity = Polarity::Inv;
                intravisit::walk_ty(self, ty);
            }
            _ => intravisit::walk_ty(self, ty),
        }
        self.polarity = outer;
    }

    fn visit_path(&mut self, path: &hir::Path<'hir>, _: hir::HirId) -> Self::Result {
        let outer = self.polarity;
        for segment in path.segments {
            let Some(arguments) = segment.args else {
                continue;
            };
            let variances = self.variances.of_arguments(self.tcx, segment.res);
            for (index, argument) in arguments.args.iter().enumerate() {
                self.polarity = self.under(variances.get(index).copied().unwrap_or(ty::Invariant));
                self.visit_generic_arg(argument);
                self.polarity = outer;
            }
            self.polarity = Polarity::Inv;
            for constraint in arguments.constraints {
                self.visit_assoc_item_constraint(constraint);
            }
            self.polarity = outer;
        }
    }

    fn visit_poly_trait_ref(&mut self, bound: &'hir hir::PolyTraitRef<'hir>) -> Self::Result {
        self.binders += 1;
        intravisit::walk_poly_trait_ref(self, bound);
        self.binders -= 1;
    }

    fn visit_lifetime(&mut self, lifetime: &'hir hir::Lifetime) -> Self::Result {
        let explicit = lifetime.syntax == hir::LifetimeSyntax::ExplicitBound;
        let name = match lifetime.kind {
            hir::LifetimeKind::Param(id) if self.own.contains_key(&id) => self.own[&id].clone(),
            hir::LifetimeKind::Static if explicit && self.binders == 0 => {
                LifetimeAnnotation::static_lifetime()
            }
            hir::LifetimeKind::ImplicitObjectLifetimeDefault => {
                return self.object_default(lifetime);
            }
            _ if !explicit && self.binders == 0 => {
                self.failure.get_or_insert(SkipReason::AnonymousLifetime);
                return;
            }
            _ => return,
        };
        if self.binders != 0 {
            self.guarded
                .push((self.slots.len(), SkipReason::HigherRanked));
        }
        self.slots.push(Slot {
            lifetime: name,
            position: self.position,
            polarity: self.polarity,
            is_self_ref: self.receiver == Some(lifetime.ident.span),
            syntax: SlotSyntax::from_rustc(lifetime.source),
            span: models::Span::from_rustc((&self.source, lifetime.ident.span)),
        });
    }
}
