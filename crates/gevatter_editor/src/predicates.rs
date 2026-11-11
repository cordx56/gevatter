use crate::TyCtxt;
use indexmap::IndexMap;

pub fn collect_predicates_locations(tcx: TyCtxt<'_>) -> IndexMap<crate::LocalDefId, crate::Edit> {
    let mut edits = IndexMap::new();
    for func in crate::collectors::collect_functions(tcx) {
        let (pos, sep) = func
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
        edits.insert(
            func.def_id,
            crate::Edit {
                krate: func.krate.clone(),
                file: func.file.clone(),
                span: models::Span::new(&func.source, pos),
                replace_with: sep.to_string(),
            },
        );
    }
    edits
}
