use editor::TemporaryFileLoader;
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
        config.file_loader = Some(Box::new(TemporaryFileLoader::new(
            config.opts.crate_name.clone(),
        )));
    }

    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        match self.current_phase {
            Phase::AnnotationFreshen => {
                tracing::info!(
                    "phase {}: annotation freshen",
                    Phase::AnnotationFreshen as u8
                );
                // ensure temporary files are cleaned on phase 1
                TemporaryFileLoader::ensure_temp_refresh();
                let edits = editor::freshen::freshen_annotations(tcx);
                tracing::info!("{} edits to apply", edits.len());
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
            Phase::AnalyzeAndFix => {
                tracing::info!("phase {}: analyze and fix", Phase::AnalyzeAndFix as u8);
                let outlive_reports = analyzer::report_outlives(tcx);
                let predicates_to_insert = editor::predicates::collect_predicates_locations(tcx);
                let edits =
                    editor::resolver::outlive_report_edits(&outlive_reports, &predicates_to_insert);
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
            Phase::Verification => {
                tracing::info!("phase {}: verification", Phase::Verification as u8);
                // TODO: verification phase!
                rustc_driver::Compilation::Continue
            }
        }
    }
}
