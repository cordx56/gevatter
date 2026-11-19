use super::GevatterDriver;
use crate::binding::BindingFile;
use crate::introduced::IntroducedFile;
use crate::report::ReportFile;
use bridge::TyCtxt;
use bridge::driver::Compilation;
use editor::TemporaryFileLoader;
use editor::simplify::Touched;
use std::collections::BTreeSet;

pub fn entrypoint(driver: &GevatterDriver, tcx: TyCtxt<'_>) -> Compilation {
    let file = ReportFile::of(tcx);
    let mut report = file.load();
    let state = BindingFile::current().load();
    let freshened = IntroducedFile::current().load();
    let fixed: BTreeSet<_> = report
        .functions
        .iter()
        .filter(|function| !function.where_added.is_empty())
        .map(|function| function.def_path.clone())
        .collect();
    let touched = Touched {
        freshened,
        fixed,
        settled: report.settled(),
        preserved: state.preserved,
    };
    let simplified = editor::simplify::simplify_signatures(tcx, &touched);
    let applied = TemporaryFileLoader::snapshot_sources().and_then(|_| {
        editor::apply_to_file(&simplified.edits)
            .or_else(|e| TemporaryFileLoader::restore_sources().and(Err(e)))
    });
    match applied {
        Ok(()) => {
            tracing::info!("simplification applied; re-exec for next phase");
            report.record_simplification(simplified.functions);
            file.store(&report);
        }
        Err(e) => tracing::warn!("failed to apply simplification: {e:?}"),
    }
    if let Err(e) = driver.re_exec(driver.current_phase.next_phase().unwrap()) {
        tracing::error!("process exec error: {e:?}");
    }
    Compilation::Stop
}
