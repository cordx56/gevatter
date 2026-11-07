use super::{naming::LifetimeNamer, *};
use crate::{
    Ident, Span, TyCtxt, editors,
    hir::{
        self, AmbigArg, Block, GenericParam, GenericParamKind, ImplItemKind, ItemKind, Node,
        TraitItemKind, Ty, TyKind,
        def::DefKind,
        intravisit::{self, Visitor},
    },
    middle,
    span::{BytePos, FileName},
};
use models::SkipReason;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn collect(tcx: TyCtxt<'_>) -> (Vec<FunctionFreshen>, Vec<ImplFreshen>) {
    let impls = collect_impls(tcx);
    let mut functions = Vec::new();
    for target in editors::collectors::collect_functions(tcx) {
        let mut bailed = target.skipped;
        if target.is_return_opaque {
            bailed.push(SkipReason::ImplTraitReturn);
        }
        let mut report = FunctionFreshen {
            def_path: target.def_path,
            introduced: Vec::new(),
            edits: Vec::new(),
            skipped: Vec::new(),
            bailed,
        };
        if report.bailed.is_empty() {
            let existing = target
                .generics
                .params
                .iter()
                .chain(
                    target
                        .parent_generics
                        .iter()
                        .flat_map(|generics| generics.params.iter()),
                )
                .filter_map(|param| {
                    matches!(param.kind, GenericParamKind::Lifetime { .. })
                        .then_some(param.name.ident().to_string())
                })
                .chain(
                    impls
                        .get(&tcx.local_parent(target.def_id))
                        .into_iter()
                        .flat_map(|imp| imp.introduced.iter().map(|l| l.to_string().to_owned())),
                );
            let mut collector = Collector {
                source: target.source.clone(),
                file: target.file.clone(),
                name: LifetimeNamer::new(existing),
                impl_names: None,
                report,
            };
            collector.visit_fn_decl(target.signature.decl);

            for predicate in target.generics.predicates {
                if let hir::WherePredicateKind::BoundPredicate(bound) = predicate.kind
                    && bound.origin == hir::PredicateOrigin::ImplTrait
                {
                    collector.skipped().visit_where_predicate(predicate);
                }
            }

            if !collector.report.introduced.is_empty() {
                let names = collector
                    .report
                    .introduced
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let (span, replace_with) = match target.generics.span_for_param_suggestion() {
                    Some(span) => (span, format!(", {names}")),
                    None if !target.generics.span.is_empty() => (
                        target
                            .generics
                            .span
                            .with_lo(target.generics.span.lo() + BytePos(1))
                            .shrink_to_lo(),
                        names,
                    ),
                    _ => (target.ident.span.shrink_to_hi(), format!("<{names}>")),
                };
                collector.push_edit(span, replace_with);
            }

            report = collector.report;
        }
        functions.push(report);
    }
    functions.sort_by(|a, b| a.def_path.cmp(&b.def_path));
    let mut impls: Vec<_> = impls.into_values().collect();
    impls.sort_by(|a, b| a.def_path.cmp(&b.def_path));
    (functions, impls)
}

pub fn collect_impls(tcx: TyCtxt<'_>) -> HashMap<LocalDefId, ImplFreshen> {
    let mut impls = HashMap::new();
    for owner in tcx.hir_crate_items(()).owners() {
        let Node::Item(item) = tcx.hir_node_by_def_id(owner.def_id) else {
            continue;
        };
        let ItemKind::Impl(imp) = item.kind else {
            continue;
        };
        if item.span.from_expansion()
            || imp
                .generics
                .params
                .iter()
                .any(|param| !matches!(param.kind, GenericParamKind::Lifetime { .. }))
        {
            continue;
        }

        let source = tcx.sess.source_map().lookup_source_file(item.span.lo());
        let FileName::Real(name) = &source.name else {
            continue;
        };
        let Some(file) = name.local_path() else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let report = FunctionFreshen {
            def_path: tcx.def_path_str(owner.def_id.to_def_id()),
            introduced: Vec::new(),
            edits: Vec::new(),
            skipped: Vec::new(),
            bailed: Vec::new(),
        };

        // avoid conflicting with existing names
        let mut used = UsedNameChecker(Vec::new());
        for item in imp.items {
            used.visit_impl_item(tcx.hir_impl_item(*item));
        }
        let existing: Vec<_> = imp
            .generics
            .params
            .iter()
            .map(|param| param.name.ident().name.to_string())
            .chain(used.0.into_iter())
            .collect();
        let mut used_checker = UsedNameChecker(existing);
        for item_id in imp.items {
            used_checker.visit_impl_item(tcx.hir_impl_item(*item_id))
        }
        let mut collector = Collector {
            source: source.clone(),
            file: file.to_path_buf(),
            name: LifetimeNamer::new(used_checker.0.iter().cloned()),
            impl_names: Some((
                models::Signature {
                    fixed: used_checker
                        .0
                        .into_iter()
                        .map(models::LifetimeAnnotation::from)
                        .collect(),
                    ..Default::default()
                },
                Vec::new(),
            )),
            report,
        };
        if let Some(trait_ref) = imp.of_trait {
            collector.visit_trait_ref(&trait_ref.trait_ref);
        }
        collector.visit_ty_unambig(imp.self_ty);
        if !collector.report.bailed.is_empty() {
            tracing::trace!(
                "collecting impl {:?} is passed due to bailed: {:?}",
                owner.def_id,
                collector.report.bailed
            );
            continue;
        }
        if collector.report.introduced.is_empty() {
            tracing::trace!(
                "collecting impl {:?} is passed due to nothing introduced",
                owner.def_id,
            );
            continue;
        }
        if collector
            .report
            .edits
            .iter()
            .any(|edit| edit.span.is_out_of_range(&text))
        {
            tracing::warn!(
                "collecting impl {:?} is passed due to out-of-range text reference; this may be caused by insufficient macro expansion check",
                owner.def_id
            );
            continue;
        }
        let names = collector
            .report
            .introduced
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let (insert, replace) = if imp.generics.span.is_empty() {
            (imp.generics.span, format!("<{names}>"))
        } else if let Some(last_param) = imp
            .generics
            .params
            .iter()
            .rfind(|p| imp.generics.span.contains(p.span))
        {
            // check trailing comma, comments, and so on
            let end = imp
                .generics
                .predicates
                .iter()
                .filter(|p| !p.kind.in_where_clause() && imp.generics.span.contains(p.span))
                .map(|p| p.span.hi())
                .fold(last_param.span.hi(), std::cmp::max);
            (
                imp.generics.span.with_lo(end).shrink_to_lo(),
                format!(", {names}"),
            )
        } else {
            // this case has bracket but empty, right?
            (
                imp.generics
                    .span
                    .with_lo(imp.generics.span.lo() + BytePos(1))
                    .shrink_to_lo(),
                names,
            )
        };
        collector.push_edit(insert, replace);
        let lo = collector
            .report
            .edits
            .iter()
            .map(|edit| edit.span.lo)
            .min()
            .unwrap();
        let hi = collector
            .report
            .edits
            .iter()
            .map(|edit| edit.span.hi)
            .max()
            .unwrap();
        let relative: Vec<_> = collector
            .report
            .edits
            .iter()
            .map(|edit| Edit {
                span: models::Span {
                    lo: edit.span.lo - lo,
                    hi: edit.span.hi - hi,
                },
                ..edit.clone()
            })
            .collect();
        let Ok(replace_with) = editors::apply(&text[lo..hi], &relative) else {
            tracing::warn!("editing impl {:?} failed; ignore", owner.def_id);
            continue;
        };
        let steps = collector.impl_names.take().unwrap().1;
        impls.insert(
            owner.def_id,
            ImplFreshen {
                def_path: collector.report.def_path,
                introduced: collector.report.introduced,
                edits: vec![Edit {
                    file: file.to_path_buf(),
                    span: models::Span { lo, hi },
                    replace_with,
                }],
                steps,
            },
        );
    }
    impls
}

struct UsedNameChecker(Vec<String>);
impl<'hir> Visitor<'hir> for UsedNameChecker {
    fn visit_generic_param(&mut self, param: &'hir GenericParam<'hir>) -> Self::Result {
        self.0.push(param.name.ident().name.to_string());
        intravisit::walk_generic_param(self, param)
    }
}

/// lifetime full freshen only targets reference and path.
/// We do not touch fn pointer, fn sugar, and dyn.
fn is_target(lifetime: &hir::Lifetime) -> bool {
    matches!(
        lifetime.syntax,
        hir::LifetimeSyntax::Implicit | hir::LifetimeSyntax::ExplicitAnonymous
    ) && matches!(
        lifetime.source,
        hir::LifetimeSource::Reference | hir::LifetimeSource::Path { .. }
    )
}
struct Collector {
    file: PathBuf,
    source: Arc<SourceFile>,
    name: naming::LifetimeNamer,
    impl_names: Option<(models::Signature, Vec<models::Step>)>,
    report: FunctionFreshen,
}
impl Collector {
    fn push_edit(&mut self, span: Span, replace_with: String) {
        let span = models::Span::new(&self.source, span);
        self.report.edits.push(Edit {
            file: self.file.clone(),
            span,
            replace_with,
        });
    }
    fn skipped(&mut self) -> SkippedCollector<'_> {
        SkippedCollector {
            source: &self.source,
            skipped: &mut self.report.skipped,
        }
    }
    fn fresh(&mut self) -> models::LifetimeAnnotation {
        let mut name = self.name.fresh();
        if let Some((signature, steps)) = &mut self.impl_names {
            signature.params.push(name);
            name = signature.params.last().unwrap().clone();
        }
        self.report.introduced.push(name.clone());
        name
    }
}
impl<'hir> Visitor<'hir> for Collector {
    fn visit_ty(&mut self, ty: &'hir Ty<'hir, AmbigArg>) -> Self::Result {
        if ty.span.from_expansion() {
            self.report.bailed.push(SkipReason::MacroExpanded);
            return;
        }
        match ty.kind {
            TyKind::FnPtr(_)
            | TyKind::TraitObject(..)
            | TyKind::OpaqueDef(_)
            | TyKind::UnsafeBinder(_) => self.skipped().visit_ty(ty),
            _ => intravisit::walk_ty(self, ty),
        }
    }
    fn visit_poly_trait_ref(&mut self, trait_ref: &'hir hir::PolyTraitRef<'hir>) -> Self::Result {
        if !trait_ref.bound_generic_params.is_empty() {
            self.skipped().visit_poly_trait_ref(trait_ref)
        } else {
            intravisit::walk_poly_trait_ref(self, trait_ref)
        }
    }
    fn visit_generic_args(&mut self, args: &'hir hir::GenericArgs<'hir>) -> Self::Result {
        if args.parenthesized != hir::GenericArgsParentheses::No {
            return self.skipped().visit_generic_args(args);
        }
        let implicit: Vec<_> = args
            .args
            .iter()
            .filter_map(|arg| match arg {
                hir::GenericArg::Lifetime(lifetime)
                    if lifetime.is_implicit() && is_target(lifetime) =>
                {
                    Some(**lifetime)
                }
                _ => None,
            })
            .collect();
        let mut inserted = false;
        for arg in args.args {
            if let hir::GenericArg::Lifetime(lifetime) = arg
                && lifetime.is_implicit()
                && is_target(*lifetime)
            {
                if !inserted {
                    let names: Vec<_> = implicit.iter().map(|_| self.fresh()).collect();
                    let (span, replace) = lifetime.suggestion(
                        &names
                            .iter()
                            .map(|l| l.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                    self.push_edit(span, replace);
                    inserted = true;
                }
                continue;
            }
            self.visit_generic_args(args);
        }
        for constraint in args.constraints {
            self.visit_assoc_item_constraint(constraint);
        }
        intravisit::walk_generic_args(self, args)
    }
    fn visit_lifetime(&mut self, lifetime: &'hir hir::Lifetime) -> Self::Result {
        if lifetime.kind == hir::LifetimeKind::ImplicitObjectLifetimeDefault {
            self.skipped().visit_lifetime(lifetime)
        } else {
            let name = self.fresh();
            let (span, replace) = lifetime.suggestion(&name.to_string());
            self.push_edit(span, replace);
        }
    }
}

struct SkippedCollector<'a> {
    source: &'a SourceFile,
    skipped: &'a mut Vec<Skipped>,
}
impl<'hir> Visitor<'hir> for SkippedCollector<'_> {
    fn visit_lifetime(&mut self, lifetime: &'hir hir::Lifetime) -> Self::Result {
        let mut reason = Vec::new();
        if lifetime.kind == hir::LifetimeKind::ImplicitObjectLifetimeDefault {
            reason.push(SkipReason::ObjectLifetimeDefault);
        } else if lifetime.syntax != hir::LifetimeSyntax::ExplicitBound {
            reason.push(SkipReason::HigherRanked);
        } else {
            return intravisit::walk_lifetime(self, lifetime);
        }
        self.skipped.push(Skipped {
            start: self
                .source
                .original_relative_byte_pos(lifetime.ident.span.lo())
                .0 as usize,
            end: self
                .source
                .original_relative_byte_pos(lifetime.ident.span.hi())
                .0 as usize,
            reason,
        });
        intravisit::walk_lifetime(self, lifetime)
    }
}
