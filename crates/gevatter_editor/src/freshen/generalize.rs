use crate::text::with_leading_blank;
use crate::usage;
use bridge::hir::{
    self,
    def_id::LocalDefId,
    intravisit::{self, Visitor},
};
use bridge::span::SourceFile;
use bridge::{FromRustc, TyCtxt};
use models::Span;
use std::collections::{HashMap, HashSet};

#[derive(Default, Debug)]
pub struct Plan {
    pub split: HashSet<LocalDefId>,
    pub removals: Vec<Span>,
}

pub fn plan<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
    generics: &'tcx hir::Generics<'tcx>,
    decl: &'tcx hir::FnDecl<'tcx>,
    source: &SourceFile,
    text: &str,
) -> Option<Plan> {
    let regions: Vec<_> = generics
        .predicates
        .iter()
        .map(|predicate| match predicate.kind {
            hir::WherePredicateKind::RegionPredicate(region) => Some(region),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let mut occurrences = Occurrences::default();
    occurrences.visit_fn_decl(decl);
    let in_body = usage::lifetime_usage(tcx, def_id, decl).in_body;
    let free = |id: &LocalDefId| !occurrences.blocked.contains(id) && !in_body.contains(id);

    let own: Vec<_> = generics
        .params
        .iter()
        .filter(|param| {
            matches!(
                param.kind,
                hir::GenericParamKind::Lifetime {
                    kind: hir::LifetimeParamKind::Explicit
                }
            )
        })
        .map(|param| param.def_id)
        .collect();
    let bound: HashSet<_> = regions
        .iter()
        .flat_map(|region| {
            let bounds = region.bounds.iter().filter_map(|bound| match bound {
                hir::GenericBound::Outlives(lifetime) => Some(*lifetime),
                _ => None,
            });
            std::iter::once(region.lifetime).chain(bounds)
        })
        .filter_map(|lifetime| match lifetime.kind {
            hir::LifetimeKind::Param(id) if own.contains(&id) => Some(id),
            _ => None,
        })
        .collect();
    let removable = bound.iter().all(free);

    let split: HashSet<_> = own
        .iter()
        .copied()
        .filter(|id| free(id) && (removable || !bound.contains(id)))
        .filter(|id| occurrences.plain.get(id).is_some_and(|count| *count > 1))
        .collect();
    let removals = if removable {
        predicate_spans(generics, source, text)
    } else {
        Vec::new()
    };
    (!split.is_empty() || !removals.is_empty()).then_some(Plan { split, removals })
}

fn predicate_spans(generics: &hir::Generics<'_>, source: &SourceFile, text: &str) -> Vec<Span> {
    let offsets = |span| Span::from_rustc((source, span));
    let (clause, inline): (Vec<&hir::WherePredicate<'_>>, Vec<_>) = generics
        .predicates
        .iter()
        .partition(|predicate| predicate.kind.in_where_clause());
    let clause =
        (!clause.is_empty()).then(|| with_leading_blank(text, offsets(generics.where_clause_span)));
    let bounds = generics.params.iter().filter_map(|param| {
        let hi = inline
            .iter()
            .filter(|predicate| match predicate.kind {
                hir::WherePredicateKind::RegionPredicate(region) => {
                    region.lifetime.kind == hir::LifetimeKind::Param(param.def_id)
                }
                _ => false,
            })
            .map(|predicate| offsets(predicate.span).hi)
            .max()?;
        Some(Span {
            lo: offsets(param.span).hi,
            hi,
        })
    });
    clause.into_iter().chain(bounds).collect()
}

#[derive(Default, Debug)]
struct Occurrences {
    plain: HashMap<LocalDefId, usize>,
    blocked: HashSet<LocalDefId>,
    binders: usize,
}
impl Occurrences {
    fn under(&mut self, binder: bool, walk: impl FnOnce(&mut Self)) {
        self.binders += usize::from(binder);
        walk(self);
        self.binders -= usize::from(binder);
    }
}
impl<'hir> Visitor<'hir> for Occurrences {
    fn visit_ty(&mut self, ty: &'hir hir::Ty<'hir, hir::AmbigArg>) -> Self::Result {
        let binder = matches!(
            ty.kind,
            hir::TyKind::FnPtr(_)
                | hir::TyKind::TraitObject(..)
                | hir::TyKind::OpaqueDef(_)
                | hir::TyKind::UnsafeBinder(_)
        );
        self.under(binder, |this| intravisit::walk_ty(this, ty))
    }
    fn visit_poly_trait_ref(&mut self, trait_ref: &'hir hir::PolyTraitRef<'hir>) -> Self::Result {
        let binder = !trait_ref.bound_generic_params.is_empty();
        self.under(binder, |this| {
            intravisit::walk_poly_trait_ref(this, trait_ref)
        })
    }
    fn visit_generic_args(&mut self, args: &'hir hir::GenericArgs<'hir>) -> Self::Result {
        let binder = args.parenthesized != hir::GenericArgsParentheses::No;
        self.under(binder, |this| intravisit::walk_generic_args(this, args))
    }
    fn visit_lifetime(&mut self, lifetime: &'hir hir::Lifetime) -> Self::Result {
        let hir::LifetimeKind::Param(id) = lifetime.kind else {
            return;
        };
        let plain = self.binders == 0
            && lifetime.syntax == hir::LifetimeSyntax::ExplicitBound
            && matches!(
                lifetime.source,
                hir::LifetimeSource::Reference | hir::LifetimeSource::Path { .. }
            );
        if plain {
            *self.plain.entry(id).or_default() += 1;
        } else {
            self.blocked.insert(id);
        }
    }
}
