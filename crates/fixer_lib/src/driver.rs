use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

use crate::rustc_driver::Callbacks;
use crate::rustc_interface::interface;
use crate::rustc_middle::ty::TyCtxt;

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
}
impl FixerRustc {
    pub fn new(args: &[String]) -> Self {
        Self {
            args: args.to_vec(),
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
            let value = std::env::var("LIFETIME_FIX_REQUEST").ok();
            sess.env_depinfo.lock().insert((
                rustc_span::Symbol::intern("LIFETIME_FIX_REQUEST"),
                value.as_deref().map(rustc_span::Symbol::intern),
            ));
        }));
    }

    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        let current_phase = Phase::current_phase();
        match current_phase {
            Some(Phase::Verification) => {
                // nothing to do
            }
            _ => {
                tracing::info!("start freshen phase");
                let edits = editors::freshen::freshen_annotations(tcx);
                tracing::info!("{} edits to apply", edits.len());
                if let Err(e) = editors::apply_to_file(&edits) {
                    tracing::warn!("file edit error: {e:?}");
                }
                if let Err(e) = self.re_exec(Phase::AnnotationFreshen.next_phase().unwrap()) {
                    tracing::error!("process exec error: {e:?}");
                }
            }
        }
        rustc_driver::Compilation::Continue
    }
}
