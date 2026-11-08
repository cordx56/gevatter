use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

use crate::middle::ty::TyCtxt;
use crate::rustc_driver::Callbacks;
use crate::rustc_interface::interface;
use crate::toolchain;

pub use rustc_driver::{catch_with_exit_code, compiler_entrypoint};

use crate::editors;

pub mod phase;

use phase::Phase;

pub fn run(args: &[String], callbacks: &mut (dyn Callbacks + Send)) -> ExitCode {
    catch_with_exit_code(|| {
        compiler_entrypoint(args, callbacks);
    })
}

pub struct DefaultRustc;

impl Callbacks for DefaultRustc {}

pub struct FixerRustc {
    args: Vec<String>,
    current_phase: Phase,
}
impl FixerRustc {
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
impl Callbacks for FixerRustc {
    fn config(&mut self, config: &mut interface::Config) {
        config.track_state = Some(Box::new(|sess| {
            let value = std::env::var(phase::ENV_VAR_NAME).ok();
            sess.env_depinfo.lock().insert((
                rustc_span::Symbol::intern(phase::ENV_VAR_NAME),
                value.as_deref().map(rustc_span::Symbol::intern),
            ));
        }));
        config.file_loader = Some(Box::new(editors::TemporaryFileLoader));
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
                let edits = editors::freshen::freshen_annotations(tcx);
                tracing::info!("{} edits to apply", edits.len());
                if let Err(_) = editors::apply_to_file(&edits) {
                    tracing::warn!("failed to edit file");
                }
                if let Err(e) = self.re_exec(Phase::AnnotationFreshen.next_phase().unwrap()) {
                    tracing::error!("process exec error: {e:?}");
                }
            }
            Phase::AnalyzeAndFix => {
                tracing::info!("phase {}: analyze and fix", Phase::AnalyzeAndFix as u8);
            }
            Phase::Verification => {
                tracing::info!("phase {}: verification", Phase::Verification as u8);
                // TODO: verification phase!
            }
        }
        rustc_driver::Compilation::Continue
    }
}
