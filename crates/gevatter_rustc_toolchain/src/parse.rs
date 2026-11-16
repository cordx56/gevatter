pub fn parse_rustc_version(output: &str) -> Option<String> {
    output.split_whitespace().nth(1).map(ToOwned::to_owned)
}
