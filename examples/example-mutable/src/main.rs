struct RustcVersion<'a, 'b> {
    source: Option<&'a str>,
    version: &'b str,
    channel: &'b str,
}

fn get_toolchain<'a>(source: &'a str) -> RustcVersion<'a, 'a> {
    let data: Vec<_> = source.split(" ").collect();
    RustcVersion {
        source: Some(source),
        version: data[1],
        channel: data[2],
    }
}

fn update<'a>(toolchain: &'a mut RustcVersion<'a, 'a>, version: &'a str) {
    toolchain.version = version;
    toolchain.source = None;
}

fn main() {
    let data = String::from("rustc 1.100.0 stable");
    let mut toolchain = get_toolchain(&data);
    update(&mut toolchain, "1.100.0");
    println!("{} {}", toolchain.channel, toolchain.version);
}
