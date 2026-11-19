use editor::TemporaryFileLoader;
use models::{CrateReport, FunctionKey, HeaderEdits};
use std::collections::BTreeSet;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

use crate::binding::{BindingFile, BindingState};
use crate::edits::EditsFile;
use crate::introduced::IntroducedFile;
use crate::report::ReportFile;
use bridge::TyCtxt;
use bridge::driver::{Callbacks, catch_fatal_errors, catch_with_exit_code, run_compiler};
use bridge::errors::{DiagCtxt, emitter::SilentEmitter};
use bridge::hir::def_id::LOCAL_CRATE;
use bridge::interface::interface;
use bridge::span;

pub mod analyze;
mod emitter;
pub mod freshen;
pub mod phase;
pub mod simplify;
pub mod verification;

use emitter::{Gate, GatedEmitter};
use phase::Phase;

pub struct DefaultRustc {
    args: Vec<String>,
}
impl DefaultRustc {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }
    pub fn run(mut self) -> ExitCode {
        let args = &std::mem::take(&mut self.args);
        catch_with_exit_code(|| {
            run_compiler(args, &mut self);
        })
    }
}
impl Callbacks for DefaultRustc {}

pub struct GevatterDriver {
    args: Vec<String>,
    current_phase: Phase,
    verified: Option<ReportFile>,
    diag_gate: Gate,
    /// only checked on lint; check whether error count grows
    denied: Option<bool>,
}
impl GevatterDriver {
    /// Pass original arguments for this function;
    /// this args are used in re-exec for next phase.
    pub fn new(original_args: Vec<String>) -> Self {
        let current_phase = Phase::current_phase().unwrap_or(Phase::AnnotationFreshen(0));
        Self {
            args: original_args,
            current_phase,
            verified: None,
            diag_gate: Gate::default(),
            denied: None,
        }
    }
    /// Run the compiler; pass edited arguments for passing the compiler (if needed)
    pub fn run(mut self, args: &[String]) -> ExitCode {
        let compiled = catch_fatal_errors(|| run_compiler(args, &mut self)).is_ok();
        if let Some(denied) = self.denied {
            if denied {
                return ExitCode::FAILURE;
            } else {
                return ExitCode::SUCCESS;
            }
        }
        if compiled {
            if self.verified.is_some() && *phase::LINT {
                tracing::info!("edited sources are compiled; re-exec for lints");
                if let Err(e) = self.re_exec(Phase::Lint) {
                    tracing::error!("process exec error: {e:?}");
                }
            }
            return ExitCode::SUCCESS;
        }
        if let Some(file) = self.verified.take() {
            let mut report = file.load();
            report.verified = false;
            if report.is_simplified() {
                tracing::warn!(
                    "compilation failed after verification; retry without simplification"
                );
                self.revert_simplification(&file, report);
            } else {
                tracing::warn!("compilation failed after verification");
                file.store(&report);
                self.fall_back();
            }
        } else if !matches!(self.current_phase, Phase::Fallback | Phase::Lint) {
            tracing::warn!("phase {} is aborted", self.current_phase);
            self.fall_back();
        }
        ExitCode::FAILURE
    }

    fn fall_back(&self) {
        let generalized = IntroducedFile::current().generalized();
        if generalized.is_empty() {
            tracing::warn!("fall back to the original");
            if let Err(e) = self.re_exec(Phase::Fallback) {
                tracing::error!("process exec error: {e:?}");
            }
        } else {
            tracing::warn!("retry without generalization");
            let bindings = BindingFile::current();
            self.restart_without_generalization(&bindings, bindings.load(), generalized);
        }
    }
    fn restart_without_generalization(
        &self,
        bindings: &BindingFile,
        mut state: BindingState,
        functions: BTreeSet<FunctionKey>,
    ) {
        tracing::info!(
            "{} functions keep their written lifetimes; restart",
            functions.len(),
        );
        state.ungeneralized.extend(functions);
        bindings.store(&state);
        if let Err(e) = self.re_exec(Phase::AnnotationFreshen(state.attempt + 1)) {
            tracing::error!("process exec error: {e:?}");
        }
    }
    fn revert_simplification(&self, file: &ReportFile, mut report: CrateReport) {
        let next_phase = match TemporaryFileLoader::restore_sources() {
            Ok(path) => {
                tracing::info!("unsimplified sources are restored to {path:?}");
                report.revert_simplification();
                Phase::Verification
            }
            Err(e) => {
                tracing::error!("failed to restore unsimplified sources: {e:?}");
                Phase::Fallback
            }
        };
        file.store(&report);
        if let Err(e) = self.re_exec(next_phase) {
            tracing::error!("process exec error: {e:?}");
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
                span::Symbol::intern(phase::ENV_VAR_NAME),
                value.as_deref().map(span::Symbol::intern),
            ));
        }));

        // don't forget initializing unit path;
        // this is used for storing all artifacts
        TemporaryFileLoader::ensure_unit_path(config);
        match self.current_phase {
            Phase::AnnotationFreshen(0) => {
                let path = TemporaryFileLoader::ensure_temp_refresh();
                tracing::info!("this is first phase; temp directory ({path:?}) is cleaned");
            }
            Phase::AnnotationFreshen(_) => {
                let path = TemporaryFileLoader::ensure_source_refresh();
                tracing::info!("this is a restart; source directory ({path:?}) is cleaned");
            }
            Phase::Fallback => EditsFile::current().store(&HeaderEdits::default()),
            _ => {}
        }

        config.register_lints = Some(Box::new(|_, store| lint::register_lints(store)));
        config.override_queries = Some(lint::override_queries);
        if *phase::ALLOW_FIXABLE {
            config.opts.lint_opts.push((
                lint::FIXABLE_LIFETIME_ERRORS.name_lower(),
                bridge::lint::Level::Warn,
            ));
        }
        if *phase::ALLOW_PERMISSIVE {
            config.opts.lint_opts.push((
                lint::NON_PERMISSIVE_LIFETIMES.name_lower(),
                bridge::lint::Level::Allow,
            ));
        }

        match self.current_phase {
            Phase::Fallback => {}
            Phase::Verification if !*phase::LINT => {}
            Phase::Lint => {
                // ignore all other error/warn than Gevatter
                let gate = self.diag_gate.clone();
                config.psess_created = Some(Box::new(move |psess| GatedEmitter::init(psess, gate)));
            }
            _ => {
                // ignore all diags on other than verification, lint, or fallback phase
                config.psess_created = Some(Box::new(|psess| {
                    *psess = bridge::session::parse::ParseSess::with_dcx(
                        DiagCtxt::new(Box::new(SilentEmitter)),
                        psess.clone_source_map(),
                    );
                }));
            }
        }

        if !matches!(self.current_phase, Phase::Fallback | Phase::Lint) {
            config.file_loader = Some(Box::new(TemporaryFileLoader));
        }
    }

    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> bridge::driver::Compilation {
        let krate = tcx.crate_name(LOCAL_CRATE);
        tracing::info!(
            "phase {} ({:?}) for crate {krate}",
            self.current_phase,
            self.current_phase,
        );

        match self.current_phase {
            Phase::AnnotationFreshen(attempt) => freshen::entrypoint(self, tcx, attempt),
            Phase::AnalyzeAndFix(_) => analyze::entrypoint(self, tcx),
            Phase::Simplify => simplify::entrypoint(self, tcx),
            Phase::Verification => verification::entrypoint(self, tcx),
            Phase::Fallback => bridge::driver::Compilation::Continue,
            Phase::Lint => {
                let lints = lint::SignatureLints::new(tcx, &EditsFile::current().load());
                let has_error = catch_fatal_errors(|| tcx.ensure_ok().analysis(())).is_err()
                    || tcx.dcx().has_errors().is_some();
                if !has_error {
                    self.diag_gate.pass(|| lints.emit(tcx));
                    return bridge::driver::Compilation::Continue;
                }

                if lints.fixes_errors() {
                    let errors = tcx.dcx().err_count();
                    self.diag_gate.pass(|| lints.emit(tcx));
                    self.denied = Some(tcx.dcx().err_count() > errors);
                } else {
                    tracing::warn!("the original has errors which are not fixed by the edits");
                    if let Err(e) = self.re_exec(Phase::Fallback) {
                        tracing::error!("process exec error: {e:?}");
                    }
                }
                bridge::driver::Compilation::Stop
            }
        }
    }
}
