use crate::json;
use models::{FunctionKey, HeaderEdits, Headers, Signature};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct HeadersFile {
    path: PathBuf,
}
impl HeadersFile {
    pub fn current() -> Self {
        Self {
            path: editor::files::headers_path(),
        }
    }

    pub fn load(&self) -> Headers {
        json::load(&self.path).unwrap_or_else(|e| {
            tracing::warn!("failed to load original headers {:?}: {e:?}", self.path);
            Headers::default()
        })
    }

    pub fn store(&self, headers: &Headers) {
        if let Err(e) = json::store(&self.path, headers) {
            tracing::error!("failed to store original headers {:?}: {e:?}", self.path);
        }
    }
}

#[derive(Clone, Debug)]
pub struct EditsFile {
    path: PathBuf,
}
impl EditsFile {
    pub fn current() -> Self {
        Self {
            path: editor::files::edits_path(),
        }
    }

    pub fn load(&self) -> HeaderEdits {
        json::load(&self.path).unwrap_or_else(|e| {
            tracing::warn!("failed to load edits {:?}: {e:?}", self.path);
            HeaderEdits::default()
        })
    }

    pub fn store(&self, edits: &HeaderEdits) {
        if let Err(e) = json::store(&self.path, edits) {
            tracing::error!("failed to store edits {:?}: {e:?}", self.path);
        }
    }
}

pub type Signatures = BTreeMap<FunctionKey, Signature>;

#[derive(Clone, Debug)]
pub struct SignaturesFile {
    path: PathBuf,
}
impl SignaturesFile {
    pub fn current() -> Self {
        Self {
            path: editor::files::signatures_path(),
        }
    }

    pub fn load(&self) -> Signatures {
        json::load(&self.path).unwrap_or_else(|e| {
            tracing::warn!("failed to load original signatures {:?}: {e:?}", self.path);
            Signatures::default()
        })
    }

    pub fn store(&self, signatures: &Signatures) {
        if let Err(e) = json::store(&self.path, signatures) {
            tracing::error!("failed to store original signatures {:?}: {e:?}", self.path);
        }
    }
}
