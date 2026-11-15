//! These error emitter ([`GatedEmitter`] and its [`Gate`]) are intended to filter errors;
//! Gevatter only shows Gevatter generated error/warns by default

use bridge::errors::emitter::{Emitter, SilentEmitter};
use bridge::errors::{DiagCtxt, DiagInner, Level};
use bridge::session::parse::ParseSess;
use bridge::span::source_map::SourceMap;
use std::env;
use std::path::Path;
use std::sync::{
    Arc, LazyLock,
    atomic::{AtomicBool, Ordering},
};

pub static LINT_ONLY: LazyLock<bool> = LazyLock::new(|| {
    !env::var("GEVATTER_LINT_ONLY").is_ok_and(|v| matches!(v.as_str(), "0" | "false"))
});

#[derive(Clone, Debug)]
pub struct Gate(Arc<AtomicBool>);
impl Gate {
    pub fn pass<T>(&self, emit: impl FnOnce() -> T) -> T {
        self.0.store(true, Ordering::Relaxed);
        let emitted = emit();
        self.0.store(!*LINT_ONLY, Ordering::Relaxed);
        emitted
    }
    fn is_open(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
impl Default for Gate {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(!*LINT_ONLY)))
    }
}

pub struct GatedEmitter {
    original: ParseSess,
    gate: Gate,
}
impl GatedEmitter {
    pub fn init(psess: &mut ParseSess, gate: Gate) {
        let silent = ParseSess::with_dcx(
            DiagCtxt::new(Box::new(SilentEmitter)),
            psess.clone_source_map(),
        );
        let original = std::mem::replace(psess, silent);
        psess.dcx().set_emitter(Box::new(Self { original, gate }));
    }
}
impl Emitter for GatedEmitter {
    fn emit_diagnostic(&mut self, diag: DiagInner) {
        if self.gate.is_open() || matches!(diag.level(), Level::Bug | Level::Fatal) {
            self.original.dcx().emit_diagnostic(diag);
        }
    }
    fn emit_artifact_notification(&mut self, path: &Path, artifact_type: &str) {
        self.original
            .dcx()
            .emit_artifact_notification(path, artifact_type);
    }
    fn source_map(&self) -> Option<&SourceMap> {
        Some(self.original.source_map())
    }
}
