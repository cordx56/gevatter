use crate::{FunctionKey, Span};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HeaderKind {
    Function,
    Impl,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct ItemHeader {
    pub kind: HeaderKind,
    pub def_path: String,
    pub file: PathBuf,
    pub span: Span,
    pub text: String,
    pub parent: Option<FunctionKey>,
}

pub type Headers = BTreeMap<FunctionKey, ItemHeader>;

/// Simple data structure for describing edits;
/// Note: internal edits are mentioned in `gevatter_editor` crate;
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Replacement {
    pub span: Span,
    pub replace_with: String,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct HeaderEdit {
    pub kind: HeaderKind,
    pub def_path: String,
    pub file: PathBuf,
    pub span: Span,
    pub original: String,
    pub replaced: String,
    pub replacements: Vec<Replacement>,
    pub parent: Option<FunctionKey>,
}

pub type HeaderEdits = BTreeMap<FunctionKey, HeaderEdit>;
