/// Common models which are used in Gevatter crates
use serde::{Deserialize, Serialize};

mod edit;
mod naming;
mod reports;
mod signature;

pub use edit::*;
pub use naming::*;
pub use reports::*;
pub use signature::*;

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
    AnonymousLifetime,
    BoundPredicate,
    LifetimeInBody,
    Unknown,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy, Debug)]
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
    pub fn static_lifetime() -> Self {
        Self::from("static")
    }
    pub fn anonymous() -> Self {
        Self::from("_")
    }
    pub fn is_static(&self) -> bool {
        self.0 == "static"
    }
    pub fn is_anonymous(&self) -> bool {
        self.0 == "_"
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
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct LifetimeRelation {
    pub shorter: LifetimeAnnotation,
    pub longer: LifetimeAnnotation,
}
impl LifetimeRelation {
    pub fn mentions(&self, lifetime: &LifetimeAnnotation) -> bool {
        &self.shorter == lifetime || &self.longer == lifetime
    }
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
