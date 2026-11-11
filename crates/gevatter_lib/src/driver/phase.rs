pub const ENV_VAR_NAME: &str = "FIXER_PHASE";

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum Phase {
    AnnotationFreshen,
    AnalyzeAndFix(usize),
    Verification,
}
impl Phase {
    pub fn from_str(s: &str) -> Option<Self> {
        let split: Vec<_> = s.split(".").collect();
        match split.len() {
            1 => match split[0] {
                "1" => Some(Phase::AnnotationFreshen),
                "2" => Some(Phase::AnalyzeAndFix(0)),
                "3" => Some(Phase::Verification),
                _ => None,
            },
            2 => match split[0] {
                "2" => split[1].parse().ok().map(|v| Phase::AnalyzeAndFix(v)),
                _ => None,
            },
            _ => None,
        }
    }
}
impl Phase {
    pub fn current_phase() -> Option<Self> {
        std::env::var(ENV_VAR_NAME)
            .map(|val| Phase::from_str(&val))
            .ok()
            .flatten()
    }
    pub fn next_phase(&self) -> Option<Self> {
        match self {
            Phase::AnnotationFreshen => Some(Phase::AnalyzeAndFix(0)),
            Phase::AnalyzeAndFix(_) => Some(Phase::Verification),
            Phase::Verification => None,
        }
    }
}
impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::AnnotationFreshen => f.write_str("1"),
            Phase::AnalyzeAndFix(sub) => f.write_str(&format!("2.{sub}")),
            Phase::Verification => f.write_str("3"),
        }
    }
}
