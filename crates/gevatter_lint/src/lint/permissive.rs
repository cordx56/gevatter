use super::SignatureLint;
use bridge::lint;

lint::declare_tool_lint! {
    pub gevatter::NON_PERMISSIVE_LIFETIMES,
    Warn,
    "there are more permissive lifetime annotations for the function.",
    report_in_external_macro: false
}

#[derive(Clone, Copy, Debug)]
pub struct NonPermissiveLifetimes;

impl SignatureLint for NonPermissiveLifetimes {
    fn lint(&self) -> &'static lint::Lint {
        NON_PERMISSIVE_LIFETIMES
    }
    fn help(&self) -> String {
        "use more permissive lifetime annotations".to_string()
    }

    fn message(&self, function: &str) -> String {
        format!("lifetime annotations of `{function}` are more restrictive than necessary")
    }
}
