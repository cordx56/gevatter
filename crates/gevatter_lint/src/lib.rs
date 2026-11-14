#![feature(rustc_private)]

use models::{FunctionKey, HeaderEdit, HeaderEdits, Headers};

mod diff;
mod header;

pub use diff::{Tokenized, replacements};
pub use header::headers;

pub fn header_edits(original: &Headers, current: &Headers) -> HeaderEdits {
    let changed = |key: &FunctionKey| {
        original
            .get(key)
            .zip(current.get(key))
            .filter(|(from, to)| from.text != to.text)
    };
    let edits: HeaderEdits = original
        .keys()
        .filter_map(|key| {
            let (from, to) = changed(key)?;
            let edit = HeaderEdit {
                kind: from.kind,
                def_path: to.def_path.clone(),
                file: from.file.clone(),
                span: from.span,
                original: from.text.clone(),
                replaced: to.text.clone(),
                replacements: replacements(&from.text, &to.text, from.span.lo),
                parent: from
                    .parent
                    .clone()
                    .filter(|parent| changed(parent).is_some()),
            };
            Some((key.clone(), edit))
        })
        .collect();
    tracing::info!("{} of {} headers are edited", edits.len(), original.len());
    edits
}
