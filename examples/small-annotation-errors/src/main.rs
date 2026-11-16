struct Two<'x, 'y> {
    pub x: &'x str,
    pub y: &'y str,
}
fn switch<'a>(t: &'a Two, flag: bool) -> &'a str {
    if flag { t.x } else { t.y }
}
fn first<'a>(t: Two<'a, 'a>) -> &'a str {
    t.x
}

fn main() {
    let x = "foo";
    let mut t = Two { x, y: x };
    let r = switch(&t, true);
    t.x = "z";
    println!("{r}");
}
