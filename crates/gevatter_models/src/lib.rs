/// Common models which are used in Gevatter crates
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[serde(transparent)]
pub struct Region(u32);
impl Region {
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}
impl From<u32> for Region {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("'?")?;
        f.write_str(&self.0.to_string())
    }
}
impl std::fmt::Debug for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Region({})", self)
    }
}

#[derive(Clone, Debug)]
pub struct RegionName(String);
impl From<&str> for RegionName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
impl std::fmt::Display for RegionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("'{}", self.0))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RegionKind {
    Static,
    EarlyBound,
    LateBound,
    Body,
    Anonymous,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    WellFormednessErrors,
    ObjectLifetimeDefault,
    HigherRanked,
    ImplTraitReturn,
    AsyncFn,
    TraitItem,
    TypeParameters,
    UnsafeBody,
    MacroExpanded,
    /// rustc can refer files something we should not edit;
    /// e.g. outside of workspace or inside target dir
    ReadOnlyFile,
    PreserveBinding,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub lo: usize,
    pub hi: usize,
}
impl Span {
    pub fn is_out_of_range(&self, text: &str) -> bool {
        text.len() <= self.lo || text.len() < self.hi
    }
}

/// Named lifetime annotation
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
#[serde(into = "String", from = "String")]
pub struct LifetimeAnnotation(String);
impl LifetimeAnnotation {
    /// Do NOT use this to print lifetime annotations!
    pub fn reveal(&self) -> &str {
        &self.0
    }
}
impl From<&str> for LifetimeAnnotation {
    fn from(value: &str) -> Self {
        Self(value.strip_prefix("'").unwrap_or(value).to_string())
    }
}
impl From<String> for LifetimeAnnotation {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}
impl From<LifetimeAnnotation> for String {
    fn from(value: LifetimeAnnotation) -> Self {
        value.to_string()
    }
}
impl std::fmt::Display for LifetimeAnnotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}", self.0)
    }
}

/// Relationship between lifetime annotations
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LifetimeRelation {
    pub shorter: LifetimeAnnotation,
    pub longer: LifetimeAnnotation,
}
impl std::fmt::Display for LifetimeRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.longer, self.shorter)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedRegion {
    pub region: Region,
    pub name: Option<LifetimeAnnotation>,
    pub kind: RegionKind,
}
impl std::fmt::Display for ResolvedRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.name {
            Some(name) => name.fmt(f),
            None => self.region.fmt(f),
        }
    }
}

/// Relationship between resolved region lifetimes
/// This relations is appeared in source as `'longer: 'shorter`
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RequiredOutlive {
    pub longer: ResolvedRegion,
    pub shorter: ResolvedRegion,
}
impl RequiredOutlive {
    pub fn named(&self) -> Option<LifetimeRelation> {
        self.longer
            .name
            .as_ref()
            .zip(self.shorter.name.as_ref())
            .map(|(longer, shorter)| LifetimeRelation {
                shorter: shorter.clone(),
                longer: longer.clone(),
            })
    }
}
impl std::fmt::Display for RequiredOutlive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.longer, self.shorter)
    }
}

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionReport {
    pub def_path: String,
    pub status: FunctionStatus,
    pub where_added: Vec<LifetimeRelation>,
    pub binding_change: BindingChange,
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

    pub fn conclude(&mut self, functions: Vec<FunctionReport>) {
        let mut known = self.take_functions();
        self.functions = functions
            .into_iter()
            .map(|function| {
                let where_added = known
                    .remove(&function.def_path)
                    .map(|known| known.where_added)
                    .unwrap_or_default();
                let status = match function.status {
                    FunctionStatus::Unchanged if !where_added.is_empty() => FunctionStatus::Fixed,
                    status => status,
                };
                FunctionReport {
                    status,
                    where_added,
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

/// Position in signature to interact
#[derive(Clone, Copy, Debug)]
pub enum SignaturePosition {
    Params(usize),
    Return,
}

/// TODO: description!
#[derive(Clone, Copy, Debug)]
pub enum SlotSyntax {
    Reference,
    Path,
}

/// TODO: description!
#[derive(Clone, Copy, Debug)]
pub enum Polarity {
    Pos,
    Neg,
    Inv,
}

/// TODO: description!
#[derive(Clone, Debug)]
pub struct Slot {
    pub lifetime: LifetimeAnnotation,
    pub position: SignaturePosition,
    pub polarity: Polarity,
    pub is_self_ref: bool,
    pub syntax: SlotSyntax,
    pub span: Span,
}

/// Represents function/method's signature
#[derive(Default, Clone, Debug)]
pub struct Signature {
    pub params: Vec<LifetimeAnnotation>,
    pub late_bound: Vec<LifetimeAnnotation>,
    pub fixed: Vec<LifetimeAnnotation>,
    pub slots: Vec<Slot>,
    pub predicates: Vec<LifetimeRelation>,
    pub known: Vec<LifetimeRelation>,
}

/// TODO: description!
pub enum Step {
    Equate {
        removed: LifetimeAnnotation,
        into: LifetimeAnnotation,
    },
    Substitute {
        from: LifetimeAnnotation,
        to: LifetimeAnnotation,
    },
    Drop {
        lifetime: LifetimeAnnotation,
    },
    Elide {
        lifetime: LifetimeAnnotation,
    },
    Rename {
        from: LifetimeAnnotation,
        to: LifetimeAnnotation,
    },
}
