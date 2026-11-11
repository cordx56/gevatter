#![feature(rustc_private)]

pub extern crate rustc_borrowck;
pub extern crate rustc_hir;
pub extern crate rustc_middle;
pub extern crate rustc_span;

pub use middle::ty::TyCtxt;
pub use rustc_borrowck::consumers as borrowck;
pub use rustc_hir as hir;
pub use rustc_middle as middle;
pub use rustc_span as span;

pub trait FromRustc<T> {
    fn from_rustc(value: T) -> Self;
}

impl FromRustc<(&span::SourceFile, span::Span)> for models::Span {
    fn from_rustc((source, span): (&rustc_span::SourceFile, rustc_span::Span)) -> Self {
        let lo = source.original_relative_byte_pos(span.lo()).0 as usize;
        let hi = source.original_relative_byte_pos(span.hi()).0 as usize;
        Self { lo, hi }
    }
}

impl FromRustc<borrowck::PoloniusRegionVid> for models::Region {
    fn from_rustc(value: borrowck::PoloniusRegionVid) -> Self {
        Self::from(value.as_u32())
    }
}
impl FromRustc<middle::ty::RegionVid> for models::Region {
    fn from_rustc(value: middle::ty::RegionVid) -> Self {
        Self::from(value.as_u32())
    }
}

impl FromRustc<hir::LifetimeSource> for models::SlotSyntax {
    fn from_rustc(value: hir::LifetimeSource) -> Self {
        match value {
            hir::LifetimeSource::Reference => Self::Reference,
            _ => Self::Path,
        }
    }
}
