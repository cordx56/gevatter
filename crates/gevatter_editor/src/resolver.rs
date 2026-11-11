use crate::predicates::{PredicateLocation, WhereInsertion};
use analyzer::OutliveRelationReport;
use bridge::hir::def_id::LocalDefId;
use indexmap::IndexMap;
use models::{
    BindingChange, FunctionReport, FunctionStatus, LifetimeRelation, RequiredOutlive, SkipReason,
    UnfixableReason,
};

#[derive(Clone, Debug)]
pub struct FixRound {
    pub edits: Vec<crate::Edit>,
    pub functions: Vec<FunctionReport>,
}

pub fn outlive_report_edits(
    reports: &IndexMap<LocalDefId, OutliveRelationReport>,
    locations: &IndexMap<LocalDefId, PredicateLocation>,
) -> FixRound {
    let mut edits = Vec::new();
    let mut functions = Vec::with_capacity(reports.len());
    for (def_id, report) in reports {
        let (status, where_added) = match locations.get(def_id) {
            None => {
                tracing::trace!(
                    "appropriate predicate for {} not found; skip",
                    report.def_path
                );
                continue;
            }
            Some(PredicateLocation::Skipped(_)) if report.borrowck_skipped.is_some() => (
                FunctionStatus::Unfixable(UnfixableReason::DiagnosticsRemain),
                Vec::new(),
            ),
            Some(PredicateLocation::Skipped(reasons)) => {
                (FunctionStatus::Skipped(reasons.clone()), Vec::new())
            }
            Some(PredicateLocation::Insertable(insertion)) => {
                if report.krate != insertion.krate {
                    tracing::error!(
                        "report crate {} != edit crate {}; this is a bug; we ignore this now, but should be fixed!",
                        &report.krate,
                        &insertion.krate
                    );
                    continue;
                }
                let (status, where_added) = required_relations(report);
                if matches!(status, FunctionStatus::Fixed) {
                    edits.push(where_edit(insertion, &where_added));
                }
                (status, where_added)
            }
        };
        functions.push(FunctionReport {
            def_path: report.def_path.clone(),
            status,
            where_added,
            binding_change: BindingChange::default(),
        });
    }
    FixRound { edits, functions }
}

fn required_relations(report: &OutliveRelationReport) -> (FunctionStatus, Vec<LifetimeRelation>) {
    if let Some(unnamed) = report.required.iter().find(|r| r.named().is_none()) {
        let reason = UnfixableReason::UnnamedRegion(unnamed.clone());
        return (FunctionStatus::Unfixable(reason), Vec::new());
    }
    let relations: Vec<_> = report
        .required
        .iter()
        .filter_map(RequiredOutlive::named)
        .collect();
    let status = if !relations.is_empty() {
        FunctionStatus::Fixed
    } else if report.had_diagnostics || report.borrowck_skipped.is_some() {
        FunctionStatus::Unfixable(UnfixableReason::DiagnosticsRemain)
    } else {
        FunctionStatus::Unchanged
    };
    (status, relations)
}

fn where_edit(insertion: &WhereInsertion, relations: &[LifetimeRelation]) -> crate::Edit {
    let predicates = relations
        .iter()
        .map(LifetimeRelation::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    crate::Edit {
        krate: insertion.krate.clone(),
        file: insertion.file.clone(),
        span: insertion.position,
        replace_with: format!("{}{predicates}", insertion.separator),
    }
}

pub fn verify_reports(
    reports: &IndexMap<LocalDefId, OutliveRelationReport>,
    locations: &IndexMap<LocalDefId, PredicateLocation>,
    bindings: &IndexMap<LocalDefId, BindingChange>,
) -> Vec<FunctionReport> {
    reports
        .iter()
        .map(|(def_id, report)| {
            let location = locations.get(def_id);
            let preserved = matches!(
                location,
                Some(PredicateLocation::Skipped(reasons))
                    if reasons.contains(&SkipReason::PreserveBinding)
            );
            let status = if preserved && !report.required.is_empty() {
                FunctionStatus::Unfixable(UnfixableReason::WouldChangeBinding(
                    report.required.clone(),
                ))
            } else if let Some(unnamed) = report.required.iter().find(|r| r.named().is_none()) {
                FunctionStatus::Unfixable(UnfixableReason::UnnamedRegion(unnamed.clone()))
            } else if !report.required.is_empty()
                && matches!(location, Some(PredicateLocation::Insertable(_)))
            {
                FunctionStatus::Unfixable(UnfixableReason::NoFixpoint)
            } else if !report.required.is_empty()
                || report.had_diagnostics
                || report.borrowck_skipped.is_some()
            {
                FunctionStatus::Unfixable(UnfixableReason::DiagnosticsRemain)
            } else if let Some(PredicateLocation::Skipped(reasons)) = location {
                FunctionStatus::Skipped(reasons.clone())
            } else {
                FunctionStatus::Unchanged
            };
            FunctionReport {
                def_path: report.def_path.clone(),
                status,
                where_added: Vec::new(),
                binding_change: bindings
                    .get(def_id)
                    .cloned()
                    .unwrap_or(BindingChange::Incomparable),
            }
        })
        .collect()
}
