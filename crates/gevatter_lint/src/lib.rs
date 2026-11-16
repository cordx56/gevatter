#![feature(rustc_private)]

mod diff;
mod header;
mod lint;

pub use diff::{Tokenized, replacements};
pub use header::{header_edits, headers};
pub use lint::*;
