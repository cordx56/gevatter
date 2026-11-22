use super::{GevatterDriver, phase};
use crate::binding::{self, BindingFile};
use crate::edits::{EditsFile, HeadersFile, SignaturesFile};
use crate::introduced::IntroducedFile;
use crate::report::ReportFile;
use bridge::driver::Compilation;
use bridge::{FromRustc, TyCtxt};
use editor::TemporaryFileLoader;
use models::{FunctionKey, FunctionStatus};
use std::collections::BTreeSet;

pub fn entrypoint(driver: &mut GevatterDriver, tcx: TyCtxt<'_>) -> Compilation {
    let outlive_reports = analyzer::report_outlives(tcx);
    let bindings = BindingFile::current();
    let mut state = bindings.load();
    let locations = editor::predicates::collect_predicates_locations(tcx, &state.preserved);
    let changes = state.changes(tcx);
    let file = ReportFile::of(tcx);
    let mut report = file.load();
    let settled = report.settled();
    report.conclude(editor::resolver::verify_reports(
        &outlive_reports,
        &locations,
        &changes,
    ));
    file.store(&report);
    for function in &report.functions {
        if let FunctionStatus::Unfixable(reason) = &function.status {
            tracing::warn!("{} is unfixable: {reason}", function.def_path);
        }
        if function.binding_change.is_changed() {
            tracing::info!(
                "binding of {} changed: {:?}",
                function.def_path,
                function.binding_change,
            );
        }
    }
    let regressed = report.functions.iter().any(|function| {
        matches!(function.status, FunctionStatus::Unfixable(_))
            && settled.contains(&function.def_path)
    });
    if regressed && report.is_simplified() {
        tracing::warn!("verification failed after simplification; retry without it");
        driver.revert_simplification(&file, report);
        return Compilation::Stop;
    }
    let generalized = IntroducedFile::current().generalized();
    let unfixable: BTreeSet<_> = report
        .functions
        .iter()
        .filter(|function| matches!(function.status, FunctionStatus::Unfixable(_)))
        .map(|function| function.def_path.as_str())
        .collect();
    let unpreserved = if *binding::PRESERVE_BINDING {
        state.unpreserved(tcx, &changes)
    } else {
        BTreeSet::new()
    };
    let blamed: BTreeSet<_> = changes
        .keys()
        .filter(|def_id| unfixable.contains(tcx.def_path_str(**def_id).as_str()))
        .map(|def_id| FunctionKey::from_rustc((tcx, *def_id)))
        .chain(unpreserved.iter().cloned())
        .filter(|key| generalized.contains(key))
        .collect();
    if !blamed.is_empty() {
        tracing::warn!("generalized functions are rejected; retry with their names");
        driver.restart_without_generalization(&bindings, state, blamed);
        return Compilation::Stop;
    }
    if !unpreserved.is_empty() {
        tracing::info!(
            "{} functions changed binding; restart with their originals",
            unpreserved.len(),
        );
        state.preserved.extend(unpreserved);
        bindings.store(&state);
        if let Err(e) = driver.re_exec(phase::Phase::AnnotationFreshen(state.attempt + 1)) {
            tracing::error!("process exec error: {e:?}");
        }
        return Compilation::Stop;
    }
    if report.verified {
        let original = HeadersFile::current().load();
        let current = lint::headers(tcx, &TemporaryFileLoader);
        let generalities = editor::simplify::generalities(
            &SignaturesFile::current().load(),
            &editor::simplify::resolved_signatures(tcx),
        );

        let bindings = changes
            .iter()
            .map(|(def_id, change)| (FunctionKey::from_rustc((tcx, *def_id)), change.clone()))
            .collect();
        let edits = lint::header_edits(&original, &current, &generalities, &bindings);
        EditsFile::current().store(&edits);
        if edits.is_empty() && *phase::LINT {
            tracing::info!("nothing is edited; re-exec for the original");
            if let Err(e) = driver.re_exec(phase::Phase::Lint) {
                tracing::error!("process exec error: {e:?}");
            }
            return Compilation::Stop;
        }
        driver.verified = Some(file);
        return Compilation::Continue;
    }
    tracing::warn!("verification failed");
    driver.fall_back();
    Compilation::Stop
}
