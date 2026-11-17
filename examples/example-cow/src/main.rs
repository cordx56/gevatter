use std::borrow::Cow;
fn ascii_capitalize<'a>(v: &'a Cow<'a, str>) -> Cow<'a, str> {
    v.chars()
        .enumerate()
        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
        .collect()
}
fn main() {
    let s = Cow::from("hello");
    let result = ascii_capitalize(&s);
    println!("{result}");
    drop(s);
    println!("{result}");
}
