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
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

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
            .or_insert_with(Vec::new)
            .push(edit.clone());
    }
    for (path, edits) in grouped {
        if matches!(mirror_of(path), Ok(None)) {
            tracing::warn!(
                "{path:?} is outside the workspace or a build product; {} edits dropped",
                edits.len()
            );
            continue;
        }
        let source = match TemporaryFileLoader.read_file(path) {
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
        TemporaryFileLoader.write_file(path, edited.as_bytes())?;
    }
    Ok(())
}

pub static CARGO_METADATA: LazyLock<Option<cargo_metadata::Metadata>> =
    LazyLock::new(|| cargo_metadata::MetadataCommand::new().exec().ok());
pub static WORKSPACE_ROOT: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Some(path) = CARGO_METADATA
        .as_ref()
        .map(|v| fs::canonicalize(v.workspace_root.as_std_path().to_path_buf()).ok())
        .flatten()
    {
        path
    } else {
        env::current_dir().unwrap()
    }
});
pub static TARGET_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Some(v) = env::var("CARGO_TARGET_DIR")
        .ok()
        .map(|v| fs::canonicalize(v).ok())
        .flatten()
    {
        v
    } else if let Some(path) = CARGO_METADATA
        .as_ref()
        .map(|v| fs::canonicalize(v.target_directory.as_std_path().to_path_buf()).ok())
        .flatten()
    {
        path
    } else {
        tracing::warn!("failed to read target directory; fallback to ./target");
        WORKSPACE_ROOT.join("target")
    }
});

fn edit_out_dir() -> PathBuf {
    TARGET_DIR.join("gevatter").join("source")
}
fn mirror_of(path: &Path) -> io::Result<Option<(PathBuf, PathBuf)>> {
    let original = fs::canonicalize(path)?;
    if original.starts_with(&*TARGET_DIR) {
        return Ok(None);
    }
    Ok(original
        .strip_prefix(&*WORKSPACE_ROOT)
        .ok()
        .map(|rel| (original.clone(), edit_out_dir().join(rel))))
}
fn ensure_mirror(path: &Path) -> io::Result<Option<PathBuf>> {
    let Some((original, mirror)) = mirror_of(path)? else {
        return Ok(None);
    };
    if !mirror.is_file() {
        let parent = mirror.parent().expect("a mirror lies below the out dir");
        fs::create_dir_all(parent)
            .inspect_err(|e| tracing::error!("failed to create out dir {parent:?}: {e:?}"))?;
        fs::copy(&original, &mirror)
            .inspect_err(|e| tracing::error!("failed to copy {original:?} to {mirror:?}: {e:?}"))?;
    }
    Ok(Some(mirror))
}

pub struct TemporaryFileLoader;
impl TemporaryFileLoader {
    pub fn ensure_temp_refresh() -> PathBuf {
        let out_dir = edit_out_dir();
        let _ = fs::remove_dir_all(&out_dir);
        out_dir
    }
    pub fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mirror = ensure_mirror(path)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{path:?} is outside the workspace or a build product"),
            )
        })?;
        fs::write(&mirror, bytes)
            .inspect_err(|e| tracing::error!("failed to write file {mirror:?}: {e:?}"))
    }
}
impl FileLoader for TemporaryFileLoader {
    fn file_exists(&self, path: &Path) -> bool {
        RealFileLoader.file_exists(path)
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        match ensure_mirror(path)? {
            Some(mirror) => RealFileLoader.read_file(&mirror),
            None => RealFileLoader.read_file(path),
        }
    }
    fn read_binary_file(&self, path: &Path) -> io::Result<Arc<[u8]>> {
        RealFileLoader.read_binary_file(path)
    }
    fn current_directory(&self) -> io::Result<PathBuf> {
        RealFileLoader.current_directory()
    }
}
