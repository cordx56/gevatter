use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum UnfixableReason {
    UnnamedRegion(RequiredOutlive),
    DiagnosticsRemain,
    NoFixpoint,
    WouldChangeBinding(Vec<RequiredOutlive>),
}
impl std::fmt::Display for UnfixableReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnnamedRegion(required) => write!(f, "`{required}` has an unnamed region"),
            Self::DiagnosticsRemain => f.write_str("diagnostics remain"),
            Self::NoFixpoint => f.write_str("no fixpoint"),
            Self::WouldChangeBinding(required) => {
                let required: Vec<_> = required.iter().map(RequiredOutlive::to_string).collect();
                write!(f, "`{}` would change binding", required.join(", "))
            }
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
#[serde(transparent)]
pub struct FunctionKey(String);
impl From<String> for FunctionKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl std::fmt::Display for FunctionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Default, Clone, Debug)]
pub struct Freshening {
    pub introduced: Vec<LifetimeAnnotation>,
    #[serde(default)]
    pub replaced: BTreeMap<LifetimeAnnotation, LifetimeAnnotation>,
    #[serde(default)]
    pub generalized: bool,
}
impl Freshening {
    pub fn is_empty(&self) -> bool {
        self.introduced.is_empty() && !self.generalized
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Binding {
    Early,
    Late,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Default, Clone, Debug)]
#[serde(transparent)]
pub struct SignatureBinding(Vec<Option<Binding>>);
impl FromIterator<Option<Binding>> for SignatureBinding {
    fn from_iter<T: IntoIterator<Item = Option<Binding>>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}
impl SignatureBinding {
    pub fn change_to(&self, new: &Self) -> BindingChange {
        if self.0.len() != new.0.len() {
            return BindingChange::Incomparable;
        }
        let slots = |from, to| -> Vec<usize> {
            self.0
                .iter()
                .zip(new.0.iter())
                .enumerate()
                .filter(|(_, (old, new))| **old == Some(from) && **new == Some(to))
                .map(|(slot, _)| slot)
                .collect()
        };
        let late_to_early = slots(Binding::Late, Binding::Early);
        let early_to_late = slots(Binding::Early, Binding::Late);
        match (late_to_early.is_empty(), early_to_late.is_empty()) {
            (true, true) => BindingChange::Unchanged,
            (false, true) => BindingChange::LateToEarly {
                slots: late_to_early,
            },
            (true, false) => BindingChange::EarlyToLate {
                slots: early_to_late,
            },
            (false, false) => BindingChange::Both {
                late_to_early,
                early_to_late,
            },
        }
    }
}

/// Report lifetime annotation early/late binding changes.
#[derive(Serialize, Deserialize, PartialEq, Eq, Default, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BindingChange {
    #[default]
    Unchanged,
    LateToEarly {
        slots: Vec<usize>,
    },
    EarlyToLate {
        slots: Vec<usize>,
    },
    Both {
        late_to_early: Vec<usize>,
        early_to_late: Vec<usize>,
    },
    Incomparable,
}
impl BindingChange {
    pub fn is_changed(&self) -> bool {
        !matches!(self, Self::Unchanged | Self::Incomparable)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FunctionStatus {
    Unchanged,
    Fixed,
    Unfixable(UnfixableReason),
    Skipped(Vec<SkipReason>),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Default, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Simplification {
    #[default]
    Untouched,
    Simplified {
        steps: Vec<Step>,
    },
    Reverted {
        steps: Vec<Step>,
    },
    Skipped {
        reasons: Vec<SkipReason>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionReport {
    pub def_path: String,
    pub status: FunctionStatus,
    pub where_added: Vec<LifetimeRelation>,
    pub binding_change: BindingChange,
    #[serde(default)]
    pub simplification: Simplification,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrateReport {
    pub krate: String,
    pub rounds: usize,
    pub verified: bool,
    pub functions: Vec<FunctionReport>,
}
impl CrateReport {
    pub fn new(krate: String) -> Self {
        Self {
            krate,
            rounds: 0,
            verified: false,
            functions: Vec::new(),
        }
    }

    pub fn record_round(&mut self, functions: &[FunctionReport]) {
        self.rounds += 1;
        let mut known = self.take_functions();
        for function in functions {
            known
                .entry(function.def_path.clone())
                .and_modify(|known| {
                    known.status = function.status.clone();
                    known
                        .where_added
                        .extend(function.where_added.iter().cloned());
                })
                .or_insert_with(|| function.clone());
        }
        self.functions = known.into_values().collect();
    }

    pub fn record_simplification(
        &mut self,
        simplifications: impl IntoIterator<Item = (String, Simplification)>,
    ) {
        let mut known = self.take_functions();
        for (def_path, simplification) in simplifications {
            if let Some(known) = known.get_mut(&def_path) {
                known.simplification = simplification;
            }
        }
        self.functions = known.into_values().collect();
    }

    pub fn settled(&self) -> BTreeSet<String> {
        self.functions
            .iter()
            .filter(|function| matches!(function.status, FunctionStatus::Unchanged))
            .map(|function| function.def_path.clone())
            .collect()
    }

    pub fn is_simplified(&self) -> bool {
        self.functions
            .iter()
            .any(|function| matches!(function.simplification, Simplification::Simplified { .. }))
    }

    pub fn revert_simplification(&mut self) {
        for function in &mut self.functions {
            if let Simplification::Simplified { steps } = &mut function.simplification {
                function.simplification = Simplification::Reverted {
                    steps: std::mem::take(steps),
                };
            }
        }
    }

    pub fn conclude(&mut self, functions: Vec<FunctionReport>) {
        let mut known = self.take_functions();
        self.functions = functions
            .into_iter()
            .map(|function| {
                let (where_added, simplification) = known
                    .remove(&function.def_path)
                    .map(|known| (known.where_added, known.simplification))
                    .unwrap_or_default();
                let status = match function.status {
                    FunctionStatus::Unchanged if !where_added.is_empty() => FunctionStatus::Fixed,
                    status => status,
                };
                FunctionReport {
                    status,
                    where_added,
                    simplification,
                    ..function
                }
            })
            .collect();
        self.verified = !self
            .functions
            .iter()
            .any(|function| matches!(function.status, FunctionStatus::Unfixable(_)));
    }

    fn take_functions(&mut self) -> BTreeMap<String, FunctionReport> {
        std::mem::take(&mut self.functions)
            .into_iter()
            .map(|function| (function.def_path.clone(), function))
            .collect()
    }
}
