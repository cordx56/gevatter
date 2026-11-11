#![feature(rustc_private)]

pub extern crate rustc_borrowck;
pub extern crate rustc_driver;
pub extern crate rustc_hir;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_session;
pub extern crate rustc_span;

pub use middle::ty::TyCtxt;
pub use rustc_borrowck::consumers as borrowck;
pub use rustc_driver as driver;
pub use rustc_hir as hir;
pub use rustc_interface as interface;
pub use rustc_middle as middle;
pub use rustc_session as session;
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

impl FromRustc<(TyCtxt<'_>, hir::def_id::LocalDefId)> for models::FunctionKey {
    fn from_rustc((tcx, def_id): (TyCtxt<'_>, hir::def_id::LocalDefId)) -> Self {
        Self::from(
            tcx.def_path(def_id.to_def_id())
                .to_string_no_crate_verbose(),
        )
    }
}
