use bridge::TyCtxt;
use bridge::hir::{
    self,
    def_id::LocalDefId,
    intravisit::{self, Visitor},
};
use bridge::middle;
use models::LifetimeAnnotation;
use std::collections::{BTreeSet, HashSet};

#[derive(Default, Debug)]
pub struct LifetimeUsage {
    pub in_body: HashSet<LocalDefId>,
    pub bound_inside: BTreeSet<LifetimeAnnotation>,
}

pub fn lifetime_usage<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
    decl: &'tcx hir::FnDecl<'tcx>,
) -> LifetimeUsage {
    let mut binders = Binders {
        tcx,
        usage: LifetimeUsage::default(),
        in_body: false,
    };
    binders.visit_fn_decl(decl);
    if let Some(body) = tcx.hir_maybe_body_owned_by(def_id) {
        binders.in_body = true;
        binders.visit_body(body);
    }
    binders.usage
}

struct Binders<'tcx> {
    tcx: TyCtxt<'tcx>,
    usage: LifetimeUsage,
    in_body: bool,
}
impl<'tcx> Visitor<'tcx> for Binders<'tcx> {
    type NestedFilter = middle::hir::nested_filter::OnlyBodies;
    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }
    fn visit_generic_param(&mut self, param: &'tcx hir::GenericParam<'tcx>) -> Self::Result {
        if matches!(param.kind, hir::GenericParamKind::Lifetime { .. }) {
            self.usage
                .bound_inside
                .insert(LifetimeAnnotation::from(param.name.ident().name.as_str()));
        }
        intravisit::walk_generic_param(self, param)
    }
    fn visit_lifetime(&mut self, lifetime: &'tcx hir::Lifetime) -> Self::Result {
        if let hir::LifetimeKind::Param(id) = lifetime.kind
            && self.in_body
        {
            self.usage.in_body.insert(id);
        }
    }
}
