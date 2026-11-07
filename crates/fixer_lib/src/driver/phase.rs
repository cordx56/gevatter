pub const ENV_VAR_NAME: &str = "FIXER_PHASE";
const PHASES: [(Phase, &str); 2] = [
    (Phase::AnnotationFreshen, "freshen"),
    (Phase::Verification, "verification"),
];

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum Phase {
    AnnotationFreshen = 1,
    Verification = 2,
}
impl Phase {
    pub fn current_phase() -> Option<Self> {
        std::env::var(ENV_VAR_NAME)
            .map(|val| {
                for (phase, string) in PHASES {
                    if &val == string {
                        return Some(phase);
                    }
                }
                None
            })
            .ok()
            .flatten()
    }
    pub fn next_phase(&self) -> Option<Self> {
        let mut iter = PHASES.iter();
        while let Some((phase, _)) = iter.next() {
            if self == phase {
                return iter.next().map(|(phase, _)| *phase);
            }
        }
        None
    }
}
impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (phase, string) in PHASES {
            if *self == phase {
                return f.write_str(string);
            }
        }
        unreachable!("PHASE list does not covered all phase!");
    }
}
