#![feature(rustc_private)]

pub extern crate rustc_borrowck;
pub extern crate rustc_driver;
pub extern crate rustc_hir;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_span;

pub use rustc_borrowck::consumers as borrowck;
pub use rustc_hir::{self as hir, def_id::CRATE_DEF_ID};
pub use rustc_middle::{
    self as middle,
    ty::{self, TyCtxt},
};
pub use rustc_span::{self as span, Ident, Span};

pub mod analyzers;
pub mod driver;
pub mod editors;
pub mod fix;
