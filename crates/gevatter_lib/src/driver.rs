use editor::TemporaryFileLoader;
use models::FunctionStatus;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

use crate::rustc_driver::Callbacks;
use crate::rustc_interface::interface;
use crate::{hir, middle::ty::TyCtxt};

pub use rustc_driver::{catch_with_exit_code, compiler_entrypoint};

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
}
impl GevatterDriver {
    pub fn new(args: &[String]) -> Self {
        let current_phase = Phase::current_phase().unwrap_or(Phase::AnnotationFreshen);
        Self {
            args: args.to_vec(),
            current_phase,
        }
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
        config.file_loader = Some(Box::new(TemporaryFileLoader));
    }

    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        tracing::info!(
            "phase {}: {:?}",
            self.current_phase.to_string(),
            self.current_phase
        );
        match self.current_phase {
            Phase::AnnotationFreshen => {
                // ensure temporary files are cleaned on phase 1
                TemporaryFileLoader::ensure_temp_refresh();
                let edits = editor::freshen::freshen_annotations(tcx);
                if let Err(_) = editor::apply_to_file(&edits) {
                    tracing::warn!("failed to edit file; re-exec for next phase");
                } else {
                    tracing::info!("edits applied; re-exec for next phase");
                }
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
                    .filter(|_| !round.edits.is_empty());
                let round = if next_round.is_none() && !round.edits.is_empty() {
                    tracing::warn!("no fixpoint after {} rounds", phase::MAX_FIX_ROUNDS);
                    round.without_fixpoint()
                } else {
                    round
                };
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
                // TODO: verification phase!
                rustc_driver::Compilation::Continue
            }
        }
    }
}
