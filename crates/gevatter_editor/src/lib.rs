#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::{self as hir, FnSig, Generics, def_id::LocalDefId};
use rustc_middle::{self as middle, ty::TyCtxt};
use rustc_span::{
    BytePos, FileName, Ident, SourceFile, Span,
    source_map::{FileLoader, RealFileLoader},
};

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod collectors;
pub mod freshen;
pub mod predicates;
pub mod resolver;
pub mod simplify;
pub mod strip;

/// A pending edit to apply
/// Replace range of `span` with `replace_with`
#[derive(Clone, Debug)]
pub struct Edit {
    pub krate: String,
    pub file: PathBuf,
    pub span: models::Span,
    pub replace_with: String,
}

#[derive(thiserror::Error, Debug)]
pub enum EditError {
    #[error("editing range is out of range")]
    OutOfRange,
    #[error("editing position violates char boundary")]
    CharBoundaryViolation,
    #[error("editing ranges overlaps")]
    Overlap,
}

#[derive(Debug)]
pub struct Skipped {
    pub start: usize,
    pub end: usize,
    pub reason: Vec<models::SkipReason>,
}

/// Information about a function that will be modified,
/// or once checked, but decided to be skipped.
pub struct TargetFunc<'tcx> {
    pub krate: String,
    pub file: PathBuf,
    pub source: Arc<SourceFile>,
    pub def_id: LocalDefId,
    pub def_path: String,
    pub ident: Ident,
    pub signature: FnSig<'tcx>,
    pub generics: &'tcx Generics<'tcx>,
    pub parent_generics: Option<&'tcx Generics<'tcx>>,
    pub skipped: Vec<models::SkipReason>,
    pub is_return_opaque: bool,
}

pub fn annotate_all() {}

pub fn apply(source: &str, edits: &[Edit]) -> Result<String, EditError> {
    let mut ordered: Vec<_> = edits.iter().collect();
    ordered.sort_by_key(|edit| (edit.span.lo, edit.span.hi));
    let capacity = (source.len() as i32).saturating_add(
        edits
            .iter()
            .map(|edit| edit.replace_with.len() as i32 - (edit.span.hi - edit.span.lo) as i32)
            .sum(),
    ) as usize;
    let mut result = String::with_capacity(capacity);
    let mut cursor = 0;
    for edit in ordered {
        if edit.span.is_out_of_range(source) {
            tracing::error!("applying edits failed: out-of-range!");
            return Err(EditError::OutOfRange);
        }
        if !source.is_char_boundary(edit.span.lo) || !source.is_char_boundary(edit.span.hi) {
            tracing::error!("applying edits failed: char boundary violation!");
            return Err(EditError::CharBoundaryViolation);
        }
        if edit.span.lo < cursor {
            tracing::error!("applying edits failed: edit ranges overlapped!");
            return Err(EditError::Overlap);
        }
        result.push_str(&source[cursor..edit.span.lo]);
        result.push_str(&edit.replace_with);
        cursor = edit.span.hi;
    }
    result.push_str(&source[cursor..]);
    Ok(result)
}
pub fn apply_to_file(edits: &[Edit]) -> io::Result<()> {
    let mut grouped = BTreeMap::new();
    for edit in edits {
        grouped
            .entry(&edit.file)
            .or_insert((edit.krate.clone(), Vec::new()))
            .1
            .push(edit.clone());
    }
    for (path, (krate, edits)) in grouped {
        let file_loader = TemporaryFileLoader::new(Some(krate));
        let source = match file_loader.read_file(path) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("failed to read file {path:?}: {e:?}");
                return Err(e);
            }
        };
        let edited = match apply(&source, &edits) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("failed to edit file {path:?}: {e:?}");
                return Ok(());
            }
        };
        file_loader.write_file(path, edited.as_bytes())?;
    }
    Ok(())
}

pub fn target_dir() -> PathBuf {
    use cargo_metadata::MetadataCommand;

    if let Ok(v) = env::var("CARGO_TARGET_DIR") {
        PathBuf::from(v)
    } else if let Ok(meta) = MetadataCommand::new().exec() {
        meta.target_directory.as_std_path().to_path_buf()
    } else {
        tracing::warn!("failed to read target directory; fallback to ./target");
        PathBuf::from("./target")
    }
    .join("gevatter")
}
pub fn edit_out_dir() -> PathBuf {
    target_dir().join("source")
}

pub struct TemporaryFileLoader {
    krate: Option<String>,
}
impl TemporaryFileLoader {
    pub fn new(krate: Option<String>) -> Self {
        Self { krate }
    }
    fn ensure_temp_path(&self, path: &Path) -> io::Result<PathBuf> {
        let out_dir = edit_out_dir();
        let relative = if path.is_absolute() {
            path.trim_prefix(self.current_directory()?)
        } else {
            path
        };
        let temp_path = if let Some(krate) = &self.krate {
            out_dir.join(krate)
        } else {
            out_dir
        }
        .join(relative);
        if !temp_path.is_file() {
            let parent = temp_path.parent().expect("invalid file path");
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::error!("failed to create out dir {parent:?}: {e:?}");
                return Err(e);
            }
            if let Err(e) = fs::copy(&relative, &temp_path) {
                tracing::error!("failed to copy source file to dest: {e:?}");
                return Err(e);
            }
        }
        Ok(temp_path)
    }
    pub fn ensure_temp_refresh() -> PathBuf {
        let out_dir = edit_out_dir();
        let _ = fs::remove_dir_all(&out_dir);
        out_dir
    }
    pub fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let temp_path = self.ensure_temp_path(path)?;
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(temp_path)
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("failed to open file {path:?}: {e:?}");
                return Err(e);
            }
        };
        if let Err(e) = file.write_all(bytes) {
            tracing::error!("failed to write file {path:?}: {e:?}");
            return Err(e);
        }
        Ok(())
    }
}
impl FileLoader for TemporaryFileLoader {
    fn file_exists(&self, path: &Path) -> bool {
        RealFileLoader.file_exists(path)
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        let temp_path = self.ensure_temp_path(path)?;
        fs::read_to_string(&temp_path)
    }
    fn read_binary_file(&self, path: &Path) -> io::Result<Arc<[u8]>> {
        RealFileLoader.read_binary_file(path)
    }
    fn current_directory(&self) -> io::Result<PathBuf> {
        RealFileLoader.current_directory()
    }
}
