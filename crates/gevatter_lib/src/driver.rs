use editor::TemporaryFileLoader;
use models::FunctionStatus;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

use crate::report::ReportFile;
use crate::rustc_driver::Callbacks;
use crate::rustc_interface::interface;
use bridge::{TyCtxt, hir::def_id::LOCAL_CRATE};

pub use rustc_driver::{catch_fatal_errors, catch_with_exit_code, compiler_entrypoint};

pub mod phase;

use phase::Phase;

pub fn run(args: &[String], callbacks: &mut (dyn Callbacks + Send)) -> ExitCode {
    catch_with_exit_code(|| {
        compiler_entrypoint(args, callbacks);
    })
}

pub struct DefaultRustc;

impl Callbacks for DefaultRustc {}

pub struct GevatterDriver {
    args: Vec<String>,
    current_phase: Phase,
    verified: Option<ReportFile>,
}
impl GevatterDriver {
    pub fn new(args: &[String]) -> Self {
        let current_phase = Phase::current_phase().unwrap_or(Phase::AnnotationFreshen);
        Self {
            args: args.to_vec(),
            current_phase,
            verified: None,
        }
    }
    pub fn run(mut self, args: &[String]) -> ExitCode {
        if catch_fatal_errors(|| compiler_entrypoint(args, &mut self)).is_ok() {
            return ExitCode::SUCCESS;
        }
        if let Some(file) = self.verified.take() {
            tracing::warn!("compilation failed after verification; fall back to the original");
            let mut report = file.load();
            report.verified = false;
            file.store(&report);
            if let Err(e) = self.re_exec(Phase::Fallback) {
                tracing::error!("process exec error: {e:?}");
            }
        }
        ExitCode::FAILURE
    }
    fn re_exec(&self, next_phase: Phase) -> std::io::Result<()> {
        let first = &self.args[0];
        let mut cmd = Command::new(first);
        cmd.args(&self.args[1..]);
        cmd.env(phase::ENV_VAR_NAME, next_phase.to_string());
        Err(cmd.exec())
    }
}
impl Callbacks for GevatterDriver {
    fn config(&mut self, config: &mut interface::Config) {
        config.track_state = Some(Box::new(|sess| {
            let value = std::env::var(phase::ENV_VAR_NAME).ok();
            sess.env_depinfo.lock().insert((
                rustc_span::Symbol::intern(phase::ENV_VAR_NAME),
                value.as_deref().map(rustc_span::Symbol::intern),
            ));
        }));

        // don't forget initializing unit path;
        // this is used for storing all artifacts
        TemporaryFileLoader::ensure_unit_path(&config);
        if self.current_phase == Phase::AnnotationFreshen {
            let path = TemporaryFileLoader::ensure_temp_refresh();
            tracing::info!("this is first phase; temp directory ({path:?}) is cleaned");
        }

        if self.current_phase != Phase::Fallback {
            config.file_loader = Some(Box::new(TemporaryFileLoader));
        }
    }

    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        let krate = tcx.crate_name(LOCAL_CRATE);
        tracing::info!(
            "phase {} ({:?}) for crate {krate}",
            self.current_phase.to_string(),
            self.current_phase
        );

        match self.current_phase {
            Phase::AnnotationFreshen => {
                let edits = editor::freshen::freshen_annotations(tcx);
                if let Err(_) = editor::apply_to_file(&edits) {
                    tracing::warn!("failed to edit file; re-exec for next phase");
                } else {
                    tracing::info!("edits applied; re-exec for next phase");
                }
                let file = ReportFile::of(tcx);
                file.store(&file.empty());
                if let Err(e) = self.re_exec(self.current_phase.next_phase().unwrap()) {
                    tracing::error!("process exec error: {e:?}");
                }
                rustc_driver::Compilation::Stop
            }
            Phase::AnalyzeAndFix(_) => {
                let outlive_reports = analyzer::report_outlives(tcx);
                let locations = editor::predicates::collect_predicates_locations(tcx);
                let round = editor::resolver::outlive_report_edits(&outlive_reports, &locations);
                let next_round = self
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

                let next_phase = if let Err(_) = editor::apply_to_file(&round.edits) {
                    tracing::warn!("failed to edit file; re-exec for next phase");
                    None
                } else {
                    tracing::info!("edits applied; re-exec for next phase");
                    next_round
                }
                .or_else(|| self.current_phase.next_phase())
                .unwrap();

                if let Err(e) = self.re_exec(next_phase) {
                    tracing::error!("process exec error: {e:?}");
                }
                rustc_driver::Compilation::Stop
            }
            Phase::Verification => {
                let outlive_reports = analyzer::report_outlives(tcx);
                let locations = editor::predicates::collect_predicates_locations(tcx);
                let file = ReportFile::of(tcx);
                let mut report = file.load();
                report.conclude(editor::resolver::verify_reports(
                    &outlive_reports,
                    &locations,
                ));
                file.store(&report);
                for function in &report.functions {
                    if let FunctionStatus::Unfixable(reason) = &function.status {
                        tracing::warn!("{} is unfixable: {reason}", function.def_path);
                    }
                }
                if report.verified {
                    self.verified = Some(file);
                    return rustc_driver::Compilation::Continue;
                }
                tracing::warn!("verification failed; fall back to the original");
                if let Err(e) = self.re_exec(Phase::Fallback) {
                    tracing::error!("process exec error: {e:?}");
                }
                rustc_driver::Compilation::Stop
            }
            Phase::Fallback => rustc_driver::Compilation::Continue,
        }
    }
}
