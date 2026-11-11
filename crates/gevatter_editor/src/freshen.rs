use super::*;
use bridge::TyCtxt;
use models::FunctionKey;
use std::collections::BTreeSet;

pub mod collector;
pub mod naming;

#[derive(Debug)]
pub struct FunctionFreshen {
    pub def_path: String,
    pub introduced: Vec<models::LifetimeAnnotation>,
    pub edits: Vec<super::Edit>,
    pub skipped: Vec<Skipped>,
    pub bailed: Vec<models::SkipReason>,
}

pub struct ImplFreshen {
    pub def_path: String,
    pub introduced: Vec<models::LifetimeAnnotation>,
    pub edits: Vec<Edit>,
    pub steps: Vec<models::Step>,
}

pub fn freshen_annotations(tcx: TyCtxt<'_>, preserved: &BTreeSet<FunctionKey>) -> Vec<Edit> {
    let (funcs, impls) = collector::collect(tcx, preserved);
    let edits: Vec<_> = funcs
        .into_iter()
        .filter_map(|v| v.bailed.is_empty().then_some(v.edits.into_iter()))
        .chain(impls.into_iter().map(|v| v.edits.into_iter()))
        .flatten()
        .collect();
    edits
}
