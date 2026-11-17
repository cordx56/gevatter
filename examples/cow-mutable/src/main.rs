use std::borrow::Cow;
fn ascii_capitalize<'a>(v: &'a mut Cow<'a, str>) {
    let mut iter = v.chars();
    let first = iter.next().unwrap().to_ascii_uppercase();
    *v = std::iter::once(first).chain(iter).collect();
}
fn main() {
    let mut s = Cow::from("hello");
    ascii_capitalize(&mut s);
    println!("{s}");
}
