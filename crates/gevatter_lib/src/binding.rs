use crate::json;
use bridge::FromRustc;
use bridge::TyCtxt;
use bridge::hir::def_id::LocalDefId;
use indexmap::IndexMap;
use models::{BindingChange, FunctionKey, SignatureBinding};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

pub static PRESERVE_BINDING: LazyLock<u8> = LazyLock::new(|| {
    env::var("GEVATTER_PRESERVE_BINDING")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
});

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct BindingState {
    pub attempt: usize,
    pub original: BTreeMap<FunctionKey, SignatureBinding>,
    pub preserved: BTreeSet<FunctionKey>,
    #[serde(default)]
    pub ungeneralized: BTreeSet<FunctionKey>,
}
impl BindingState {
    pub fn start_attempt(self, tcx: TyCtxt<'_>, attempt: usize) -> Self {
        let original = analyzer::binding::signature_bindings(tcx)
            .into_iter()
            .map(|(def_id, binding)| (FunctionKey::from_rustc((tcx, def_id)), binding))
            .collect();
        Self {
            attempt,
            original,
            preserved: self.preserved,
            ungeneralized: self.ungeneralized,
        }
    }

    pub fn changes(&self, tcx: TyCtxt<'_>) -> IndexMap<LocalDefId, BindingChange> {
        analyzer::binding::signature_bindings(tcx)
            .into_iter()
            .map(|(def_id, current)| {
                let change = self
                    .original
                    .get(&FunctionKey::from_rustc((tcx, def_id)))
                    .map_or(BindingChange::Incomparable, |original| {
                        original.change_to(&current)
                    });
                (def_id, change)
            })
            .collect()
    }

    pub fn unpreserved(
        &self,
        tcx: TyCtxt<'_>,
        changes: &IndexMap<LocalDefId, BindingChange>,
    ) -> BTreeSet<FunctionKey> {
        changes
            .iter()
            .filter(|(_, change)| {
                if *PRESERVE_BINDING == 0 {
                    true
                } else if *PRESERVE_BINDING == 1 {
                    change.has_own_late_to_early()
                } else if *PRESERVE_BINDING == 2 {
                    change.has_late_to_early()
                } else {
                    change.is_changed()
                }
            })
            .map(|(def_id, _)| FunctionKey::from_rustc((tcx, *def_id)))
            .filter(|key| !self.preserved.contains(key))
            .collect()
    }
}

/// Binding file is intended to carry [`BindingState`] across restarts.
/// The file path is decided by [`editor::files::binding_path`].
#[derive(Clone, Debug)]
pub struct BindingFile {
    path: PathBuf,
}
impl BindingFile {
    pub fn current() -> Self {
        Self {
            path: editor::files::binding_path(),
        }
    }

    pub fn load(&self) -> BindingState {
        json::load(&self.path).unwrap_or_else(|e| {
            tracing::warn!("failed to load binding state {:?}: {e:?}", self.path);
            BindingState::default()
        })
    }

    pub fn store(&self, state: &BindingState) {
        if let Err(e) = json::store(&self.path, state) {
            tracing::error!("failed to store binding state {:?}: {e:?}", self.path);
        }
    }
}
