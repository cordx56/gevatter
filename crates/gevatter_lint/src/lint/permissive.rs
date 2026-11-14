use super::SignatureLint;
use bridge::lint;

lint::declare_tool_lint! {
    pub gevatter::NON_PERMISSIVE_LIFETIMES,
    Warn,
    "there are more permissive lifetime annotations for the function.",
    report_in_external_macro: false
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NonPermissiveLifetimes;

impl SignatureLint for NonPermissiveLifetimes {
    const LINT: &'static lint::Lint = NON_PERMISSIVE_LIFETIMES;
    const HELP: &'static str = "use more permissive lifetime annotations";

    fn message(function: &str) -> String {
        format!("lifetime annotations of `{function}` are more restrictive than necessary")
    }
}
