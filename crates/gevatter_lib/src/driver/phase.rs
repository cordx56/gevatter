pub const ENV_VAR_NAME: &str = "FIXER_PHASE";

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum Phase {
    AnnotationFreshen = 1,
    AnalyzeAndFix = 2,
    Verification = 3,
}
impl Phase {
    pub fn from_num(v: u8) -> Option<Self> {
        match v {
            1 => Some(Phase::AnnotationFreshen),
            2 => Some(Phase::AnalyzeAndFix),
            3 => Some(Phase::Verification),
            _ => None,
        }
    }
}
impl Into<u8> for Phase {
    fn into(self) -> u8 {
        self as u8
    }
}
impl Phase {
    pub fn current_phase() -> Option<Self> {
        std::env::var(ENV_VAR_NAME)
            .map(|val| {
                let number: Option<u8> = val.parse().ok();
                number.map(Phase::from_num).flatten()
            })
            .ok()
            .flatten()
    }
    pub fn next_phase(&self) -> Option<Self> {
        Phase::from_num(*self as u8 + 1)
    }
}
impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&(*self as u8).to_string())
    }
}
