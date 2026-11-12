use std::env;
use std::sync::LazyLock;

pub const ENV_VAR_NAME: &str = "GEVATTER_PHASE";
pub static MAX_FIX_ROUNDS: LazyLock<usize> = LazyLock::new(|| {
    env::var("GEVATTER_MAX_FIX_ROUNDS")
        .ok()
        .map(|v| v.parse().ok())
        .flatten()
        .unwrap_or(10)
});

pub static SIMPLIFY: LazyLock<bool> = LazyLock::new(|| {
    !env::var("GEVATTER_SIMPLIFY").is_ok_and(|v| matches!(v.as_str(), "0" | "false"))
});

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum Phase {
    AnnotationFreshen(usize),
    AnalyzeAndFix(usize),
    Simplify,
    Verification,
    Fallback,
}
impl Phase {
    pub fn from_str(s: &str) -> Option<Self> {
        let split: Vec<_> = s.split(".").collect();
        match split.len() {
            1 => match split[0] {
                "1" => Some(Phase::AnnotationFreshen(0)),
                "2" => Some(Phase::AnalyzeAndFix(0)),
                "3" => Some(Phase::Simplify),
                "4" => Some(Phase::Verification),
                "5" => Some(Phase::Fallback),
                _ => None,
            },
            2 => match split[0] {
                "1" => split[1].parse().ok().map(Phase::AnnotationFreshen),
                "2" => split[1].parse().ok().map(Phase::AnalyzeAndFix),
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
            Phase::AnnotationFreshen(_) => Some(Phase::AnalyzeAndFix(0)),
            Phase::AnalyzeAndFix(_) if *SIMPLIFY => Some(Phase::Simplify),
            Phase::AnalyzeAndFix(_) | Phase::Simplify => Some(Phase::Verification),
            Phase::Verification => None,
            Phase::Fallback => None,
        }
    }
    /// This returns None if the current phase is not "analyze and fix",
    /// or it exceeds max fix rounds
    pub fn next_round(&self) -> Option<Self> {
        match self {
            Phase::AnalyzeAndFix(round) if round + 1 < *MAX_FIX_ROUNDS => {
                Some(Phase::AnalyzeAndFix(round + 1))
            }
            _ => None,
        }
    }
}
impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::AnnotationFreshen(sub) => f.write_str(&format!("1.{sub}")),
            Phase::AnalyzeAndFix(sub) => f.write_str(&format!("2.{sub}")),
            Phase::Simplify => f.write_str("3"),
            Phase::Verification => f.write_str("4"),
            Phase::Fallback => f.write_str("5"),
        }
    }
}
