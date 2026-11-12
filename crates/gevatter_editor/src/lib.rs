#![feature(rustc_private)]

use bridge::hir::{FnSig, Generics, def_id::LocalDefId};
use bridge::span::{Ident, SourceFile, source_map::FileLoader};

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

pub mod collectors;
pub mod files;
pub mod freshen;
pub mod predicates;
pub mod resolver;
pub mod simplify;

pub use files::TemporaryFileLoader;

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

/// Apply edits to the source string
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
/// Apply edits for each files
pub fn apply_to_file(edits: &[Edit]) -> io::Result<()> {
    tracing::info!("{} edits to apply", edits.len());
    let mut grouped = BTreeMap::new();
    for edit in edits {
        grouped
            .entry(&edit.file)
            .or_insert_with(Vec::new)
            .push(edit.clone());
    }
    for (path, edits) in grouped {
        if matches!(files::mirror_of(path), Ok(None)) {
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
                return Err(io::Error::other(e));
            }
        };
        TemporaryFileLoader.write_file(path, edited.as_bytes())?;
    }
    Ok(())
}
