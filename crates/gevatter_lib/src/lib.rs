#![feature(rustc_private)]

pub use bridge::{
    TyCtxt,
    hir::{self, def_id::CRATE_DEF_ID},
    middle::ty,
    session,
    span::{Ident, Span},
};

pub mod binding;
pub mod driver;
pub mod fix;
mod json;
pub mod report;
