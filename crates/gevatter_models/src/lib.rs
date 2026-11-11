/// Common models which are used in Gevatter crates

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
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
        f.write_str(&format!("Region({})", self.to_string()))
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

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum RegionKind {
    Static,
    EarlyBound,
    LateBound,
    Body,
    Anonymous,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
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
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
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
impl std::fmt::Display for LifetimeAnnotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("'")?;
        f.write_str(&self.0)
    }
}

/// Relationship between lifetime annotations
#[derive(Clone, Debug)]
pub struct LifetimeRelation {
    pub shorter: LifetimeAnnotation,
    pub longer: LifetimeAnnotation,
}
impl std::fmt::Display for LifetimeRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.longer, self.shorter)
    }
}

#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub enum UnfixableReason {
    UnnamedRegion(RequiredOutlive),
    DiagnosticsRemain,
    NoFixpoint,
}
impl std::fmt::Display for UnfixableReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnnamedRegion(required) => write!(f, "`{required}` has an unnamed region"),
            Self::DiagnosticsRemain => f.write_str("diagnostics remain"),
            Self::NoFixpoint => f.write_str("no fixpoint"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum FunctionStatus {
    Unchanged,
    Fixed,
    Unfixable(UnfixableReason),
    Skipped(Vec<SkipReason>),
}

#[derive(Clone, Debug)]
pub struct FunctionReport {
    pub def_path: String,
    pub status: FunctionStatus,
    pub where_added: Vec<LifetimeRelation>,
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
