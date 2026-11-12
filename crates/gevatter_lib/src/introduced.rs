use crate::json;
use models::{FunctionKey, LifetimeAnnotation};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type Introduced = BTreeMap<FunctionKey, Vec<LifetimeAnnotation>>;

#[derive(Clone, Debug)]
pub struct IntroducedFile {
    path: PathBuf,
}
impl IntroducedFile {
    pub fn current() -> Self {
        Self {
            path: editor::files::introduced_path(),
        }
    }

    pub fn load(&self) -> Introduced {
        json::load(&self.path).unwrap_or_else(|e| {
            tracing::warn!("failed to load introduced names {:?}: {e:?}", self.path);
            Introduced::default()
        })
    }

    pub fn store(&self, introduced: &Introduced) {
        if let Err(e) = json::store(&self.path, introduced) {
            tracing::error!("failed to store introduced names {:?}: {e:?}", self.path);
        }
    }
}
