use crate::{
    TyCtxt,
    hir::{
        self,
        def_id::LocalDefId,
        intravisit::{self, Visitor},
    },
    middle,
};
use std::collections::HashSet;

pub fn collect(tcx: TyCtxt<'_>) {
    for target in super::collectors::collect_functions(tcx) {
        let span = models::Span::new(&target.source, target.signature.span);
        let hi = span
            .hi
            .max(models::Span::new(&target.source, target.generics.where_clause_span).hi);

        let eligible = target.skipped.is_empty();
        let has_param: HashSet<_> = target
            .generics
            .params
            .iter()
            .filter(|p| matches!(p.kind, hir::GenericParamKind::Lifetime { .. }))
            .map(|p| p.def_id)
            .collect();
        let mut checker = SafetyChecker {
            has_param,
            protected: false,
            forbidden: false,
            used_lifetime: Vec::new(),
        };
        checker.visit_fn_decl(target.signature.decl);
        checker.visit_generics(target.generics);
        let mut skip = !target.skipped.is_empty();
        if checker.forbidden {
            tracing::trace!(
                "function `{}` has forbidden types; skip lifetime strip",
                target.def_path
            );
            skip = true;
        }
        // TODO: Simplify check
        //let changes = Vec::new();
        if !skip {
            tracing::trace!(
                "continue to lifetime strip in function `{}`",
                target.def_path
            );
            let mut body_checker = LifetimeUsageCheckerInBody {
                tcx,
                used_def_id: Default::default(),
            };
            if let Some(id) = tcx.hir_maybe_body_owned_by(target.def_id) {
                tracing::trace!(
                    "function `{}` has body; check lifetime usage",
                    target.def_path
                );
                body_checker.visit_body(id);
                tracing::trace!("lifetime usage: {:?}", body_checker.used_def_id);
            }

            let mut before = models::Signature {
                params: target
                    .generics
                    .params
                    .iter()
                    .filter(|p| p.is_lifetime())
                    .map(|p| models::LifetimeAnnotation::from(p.name.ident().name.to_string()))
                    .collect(),
                ..Default::default()
            };
            let mut occurrences = SafetyChecker {
                has_param: checker.has_param,
                protected: false,
                forbidden: false,
                used_lifetime: Default::default(),
            };
            occurrences.visit_fn_decl(target.signature.decl);
            before.slots = occurrences
                .used_lifetime
                .iter()
                .map(|lifetime| models::Slot {
                    lifetime: models::LifetimeAnnotation::from(lifetime.ident.name.to_string()),
                    position: models::SignaturePosition::Params(0),
                    polarity: models::Polarity::Inv,
                    is_self_ref: false,
                    syntax: models::SlotSyntax::from(lifetime.source),
                    span: models::Span::new(&target.source, lifetime.ident.span),
                })
                .collect();
            let mut after = before.clone();
        }
    }
}

struct SafetyChecker {
    has_param: HashSet<LocalDefId>,
    protected: bool,
    forbidden: bool,
    used_lifetime: Vec<hir::Lifetime>,
}
impl<'hir> Visitor<'hir> for SafetyChecker {
    fn visit_ty(&mut self, ty: &'hir hir::Ty<'hir, hir::AmbigArg>) -> Self::Result {
        let outer = self.protected;
        self.protected |= matches!(
            ty.kind,
            hir::TyKind::FnPtr(_) | hir::TyKind::TraitObject(_, _) | hir::TyKind::UnsafeBinder(_)
        );
        self.forbidden |= matches!(ty.kind, hir::TyKind::OpaqueDef(_));
        let visit_inner = intravisit::walk_ty(self, ty);
        self.protected = outer;
        visit_inner
    }
    fn visit_poly_trait_ref(&mut self, ty: &'hir hir::PolyTraitRef<'hir>) -> Self::Result {
        let outer = self.protected;
        self.protected = true;
        let visit_inner = intravisit::walk_poly_trait_ref(self, ty);
        self.protected = outer;
        visit_inner
    }
    fn visit_lifetime(&mut self, lifetime: &'hir hir::Lifetime) -> Self::Result {
        if let hir::LifetimeKind::Param(id) = lifetime.kind
            && self.has_param.contains(&id)
        {
            self.forbidden |= self.protected;
            if matches!(lifetime.syntax, hir::LifetimeSyntax::ExplicitBound) {
                self.used_lifetime.push(*lifetime);
            }
        }
    }
}

struct LifetimeUsageCheckerInBody<'tcx> {
    tcx: TyCtxt<'tcx>,
    used_def_id: HashSet<LocalDefId>,
}
impl<'tcx> Visitor<'tcx> for LifetimeUsageCheckerInBody<'tcx> {
    type NestedFilter = middle::hir::nested_filter::OnlyBodies;
    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }
    fn visit_lifetime(&mut self, lifetime: &'tcx hir::Lifetime) -> Self::Result {
        if let hir::LifetimeKind::Param(id) = lifetime.kind {
            self.used_def_id.insert(id);
        }
    }
}
