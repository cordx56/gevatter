use bridge::lexer::{FrontmatterAllowed, TokenKind, tokenize};
use models::{Replacement, Span};
use similar::{Algorithm, DiffTag, capture_diff_slices};

pub struct Tokenized {
    tokenized: Vec<String>,
    offsets: Vec<usize>,
}
impl Tokenized {
    pub fn new(text: &str) -> Self {
        let starts = tokenize(text, FrontmatterAllowed::No)
            .scan(0, |offset, token| {
                let start = *offset;
                *offset += token.len as usize;
                Some((start, token.kind))
            })
            .filter(|(start, kind)| *start != 0 && *kind != TokenKind::Whitespace)
            .map(|(start, _)| start);
        let offsets: Vec<_> = std::iter::once(0)
            .chain(starts)
            .chain(std::iter::once(text.len()))
            .collect();
        let tokenized = offsets
            .windows(2)
            .map(|range| text[range[0]..range[1]].to_owned())
            .collect();
        Self { tokenized, offsets }
    }
    pub fn tokens(&self) -> &[String] {
        &self.tokenized
    }
    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }
    pub fn iter(&self) -> impl Iterator {
        self.tokenized.iter()
    }
}

pub fn replacements(original: &str, replaced: &str, base: usize) -> Vec<Replacement> {
    let old = Tokenized::new(original);
    let new = Tokenized::new(replaced);
    capture_diff_slices(Algorithm::Myers, old.tokens(), new.tokens())
        .into_iter()
        .map(|op| op.as_tag_tuple())
        .filter(|(tag, _, _)| *tag != DiffTag::Equal)
        .map(|(_, from, to)| Replacement {
            span: Span {
                lo: base + old.offsets()[from.start],
                hi: base + old.offsets()[from.end],
            },
            replace_with: replaced[new.offsets[to.start]..new.offsets[to.end]].to_string(),
        })
        .collect()
}
