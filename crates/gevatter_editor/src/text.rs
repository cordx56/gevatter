use models::Span;

pub fn with_trailing_whitespace(text: &str, span: Span) -> Span {
    let blank = text[span.hi..]
        .bytes()
        .take_while(u8::is_ascii_whitespace)
        .count();
    Span {
        lo: span.lo,
        hi: span.hi + blank,
    }
}

pub fn with_leading_blank(text: &str, span: Span) -> Span {
    let is_blank = |byte: &u8| matches!(byte, b' ' | b'\t');
    let lo = span.lo - text[..span.lo].bytes().rev().take_while(is_blank).count();
    let hi = span.hi + text[span.hi..].bytes().take_while(is_blank).count();
    if text[..lo].ends_with('\n') && text[hi..].starts_with('\n') {
        Span { lo, hi: hi + 1 }
    } else {
        Span { lo, hi: span.hi }
    }
}
