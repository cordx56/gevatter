use super::*;
use bridge::TyCtxt;
use models::{FunctionKey, LifetimeAnnotation};
use std::collections::BTreeSet;

pub mod collector;

#[derive(Debug)]
pub struct FunctionFreshen {
    pub key: FunctionKey,
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

#[derive(Default, Debug)]
pub struct Freshened {
    pub edits: Vec<Edit>,
    pub introduced: BTreeMap<FunctionKey, Vec<LifetimeAnnotation>>,
}

pub fn freshen_annotations(tcx: TyCtxt<'_>, preserved: &BTreeSet<FunctionKey>) -> Freshened {
    let (funcs, impls) = collector::collect(tcx, preserved);
    let (introduced, edits): (BTreeMap<_, _>, Vec<_>) = funcs
        .into_iter()
        .filter(|func| func.bailed.is_empty())
        .map(|func| ((func.key, func.introduced), func.edits))
        .unzip();
    Freshened {
        edits: edits
            .into_iter()
            .chain(impls.into_iter().map(|imp| imp.edits))
            .flatten()
            .collect(),
        introduced: introduced
            .into_iter()
            .filter(|(_, names)| !names.is_empty())
            .collect(),
    }
}
