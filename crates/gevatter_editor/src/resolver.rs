use crate::LocalDefId;
use analyzer::OutliveRelationReport;
use indexmap::IndexMap;

pub fn outlive_report_edits(
    reports: &IndexMap<LocalDefId, OutliveRelationReport>,
    predicates: &IndexMap<LocalDefId, crate::Edit>,
) -> Vec<crate::Edit> {
    let mut inserts = Vec::with_capacity(reports.len().min(predicates.len()));
    for (def_id, report) in reports {
        let Some(edit) = predicates.get(def_id) else {
            tracing::trace!(
                "appropriate predicate for {} not found; skip",
                report.def_path
            );
            continue;
        };
        if report.krate != edit.krate {
            tracing::error!(
                "report crate {} != edit crate {}; this is a bug; we ignore this now, but should be fixed!",
                &report.krate,
                &edit.krate
            );
            continue;
        }
        let mut insert = edit.clone();
        let predicates: Vec<_> = report
            .named_required
            .iter()
            .map(|rel| format!("{}: {}", rel.longer.to_string(), rel.shorter.to_string()))
            .collect();
        insert.replace_with.push_str(&predicates.join(", "));
        inserts.push(insert);
    }
    inserts
}
