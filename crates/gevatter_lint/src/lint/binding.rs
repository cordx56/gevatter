use super::SignatureLint;
use bridge::lint;

/// Linter wrapper for late-to-early note
#[derive(Clone, Copy, Debug)]
pub struct LateToEarly<L>(pub L);

impl<L: SignatureLint> SignatureLint for LateToEarly<L> {
    fn lint(&self) -> &'static lint::Lint {
        self.0.lint()
    }
    fn help(&self) -> String {
        self.0.help()
    }

    fn message(&self, function: &str) -> String {
        self.0.message(function)
    }
    fn notes(&self) -> Vec<String> {
        let note = "this change turns late-bound lifetimes into early-bound ones; \
                    using this as a higher-ranked function may no longer compile";
        self.0
            .notes()
            .into_iter()
            .chain(std::iter::once(note.to_string()))
            .collect()
    }
}
