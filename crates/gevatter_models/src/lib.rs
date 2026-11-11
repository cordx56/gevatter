#![feature(rustc_private)]

extern crate rustc_borrowck;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_borrowck::consumers as borrowck;
use rustc_hir as hir;
use rustc_middle::ty;
use rustc_span as span;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Region(u32);
impl Region {
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}
impl From<borrowck::PoloniusRegionVid> for Region {
    fn from(value: borrowck::PoloniusRegionVid) -> Self {
        Self(value.as_u32())
    }
}
impl From<ty::RegionVid> for Region {
    fn from(value: ty::RegionVid) -> Self {
        Self(value.as_u32())
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

#[derive(Clone, Copy, Debug)]
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
    Unknown,
}

#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub lo: usize,
    pub hi: usize,
}
impl Span {
    pub fn new(source: &span::SourceFile, span: span::Span) -> Self {
        let lo = source.original_relative_byte_pos(span.lo()).0 as usize;
        let hi = source.original_relative_byte_pos(span.hi()).0 as usize;
        Self { lo, hi }
    }
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
impl From<hir::LifetimeSource> for SlotSyntax {
    fn from(value: hir::LifetimeSource) -> Self {
        match value {
            hir::LifetimeSource::Reference => Self::Reference,
            _ => Self::Path,
        }
    }
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
