use super::{GevatterDriver, phase};
use crate::binding::BindingFile;
use crate::report::ReportFile;
use bridge::TyCtxt;
use bridge::driver::Compilation;
use models::FunctionStatus;

pub fn entrypoint(driver: &GevatterDriver, tcx: TyCtxt<'_>) -> Compilation {
    let outlive_reports = analyzer::report_outlives(tcx);
    let state = BindingFile::current().load();
    let locations = editor::predicates::collect_predicates_locations(tcx, &state.preserved);
    let round = editor::resolver::outlive_report_edits(&outlive_reports, &locations);
    let next_round = driver
        .current_phase
        .next_round()
        // if edits is empty, we should go next phase;
        .filter(|_| !round.edits.is_empty());
    // if there are some edits to apply while next round is None,
    // it shows that there are edits we should apply but max fix rounds exceeded.
    if next_round.is_none() && !round.edits.is_empty() {
        tracing::warn!("no fixpoint after {} rounds", *phase::MAX_FIX_ROUNDS);
    }

    let file = ReportFile::of(tcx);
    let mut report = file.load();
    report.record_round(&round.functions);
    file.store(&report);
    for function in &round.functions {
        match &function.status {
            FunctionStatus::Unfixable(reason) => {
                tracing::warn!("{} is unfixable: {reason}", function.def_path)
            }
            status => tracing::debug!("{}: {status:?}", function.def_path),
        }
    }

    let next_phase = if editor::apply_to_file(&round.edits).is_err() {
        tracing::warn!("failed to edit file; re-exec for next phase");
        None
    } else {
        tracing::info!("edits applied; re-exec for next phase");
        next_round
    }
    .or_else(|| {
        driver
            .current_phase
            .next_phase()
            .filter(|_| round.edits.is_empty())
    })
    .unwrap_or(phase::Phase::Verification);

    if let Err(e) = driver.re_exec(next_phase) {
        tracing::error!("process exec error: {e:?}");
    }
    Compilation::Stop
}
