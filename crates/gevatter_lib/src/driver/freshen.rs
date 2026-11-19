use super::{GevatterDriver, phase};
use crate::binding::{BindingFile, BindingState};
use crate::edits::{HeadersFile, SignaturesFile};
use crate::introduced::IntroducedFile;
use crate::report::ReportFile;
use bridge::TyCtxt;
use bridge::driver::Compilation;
use editor::TemporaryFileLoader;
use editor::freshen::Generalization;

pub fn entrypoint(driver: &GevatterDriver, tcx: TyCtxt<'_>, attempt: usize) -> Compilation {
    let bindings = BindingFile::current();
    let state = match attempt {
        0 => BindingState::default(),
        _ => bindings.load(),
    }
    .start_attempt(tcx, attempt);
    bindings.store(&state);
    HeadersFile::current().store(&lint::headers(tcx, &TemporaryFileLoader));
    SignaturesFile::current().store(&editor::simplify::resolved_signatures(tcx));
    let generalization = phase::GENERALIZE.then_some(Generalization {
        excluded: state.ungeneralized,
    });
    let freshened =
        editor::freshen::freshen_annotations(tcx, &state.preserved, generalization.as_ref());
    IntroducedFile::current().store(&freshened.functions);
    if editor::apply_to_file(&freshened.edits).is_err() {
        tracing::warn!("failed to edit file; re-exec for next phase");
    } else {
        tracing::info!("edits applied; re-exec for next phase");
    }

    let file = ReportFile::of(tcx);
    file.store(&file.empty());

    if let Err(e) = driver.re_exec(driver.current_phase.next_phase().unwrap()) {
        tracing::error!("process exec error: {e:?}");
    }
    Compilation::Stop
}
