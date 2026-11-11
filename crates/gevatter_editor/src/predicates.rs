use bridge::{FromRustc, TyCtxt};
use indexmap::IndexMap;
use models::{FunctionKey, SkipReason};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct WhereInsertion {
    pub krate: String,
    pub file: PathBuf,
    pub position: models::Span,
    pub separator: &'static str,
}

#[derive(Clone, Debug)]
pub enum PredicateLocation {
    Insertable(WhereInsertion),
    Skipped(Vec<SkipReason>),
}

pub fn collect_predicates_locations(
    tcx: TyCtxt<'_>,
    preserved: &BTreeSet<FunctionKey>,
) -> IndexMap<crate::LocalDefId, PredicateLocation> {
    crate::collectors::collect_functions(tcx, preserved)
        .into_iter()
        .map(|func| {
            if func
                .skipped
                .iter()
                .any(|reason| *reason != SkipReason::UnsafeBody)
            {
                return (func.def_id, PredicateLocation::Skipped(func.skipped));
            }
            let (pos, separator) = func
                .generics
                .predicates
                .iter()
                .rfind(|p| p.kind.in_where_clause())
                .map(|last| (last.span.shrink_to_hi(), ", "))
                .unwrap_or_else(|| {
                    if func.generics.where_clause_span.is_empty() {
                        (func.generics.where_clause_span.shrink_to_lo(), " where ")
                    } else {
                        (func.generics.where_clause_span.shrink_to_hi(), " ")
                    }
                });
            let insertion = WhereInsertion {
                krate: func.krate,
                file: func.file,
                position: models::Span::from_rustc((&func.source, pos)),
                separator,
            };
            (func.def_id, PredicateLocation::Insertable(insertion))
        })
        .collect()
}
