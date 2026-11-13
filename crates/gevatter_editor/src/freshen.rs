use super::*;
use bridge::TyCtxt;
use models::{Freshening, FunctionKey, LifetimeAnnotation};
use std::collections::BTreeSet;

pub mod collector;
mod generalize;

#[derive(Clone, Debug)]
pub struct Generalization {
    pub excluded: BTreeSet<FunctionKey>,
}

#[derive(Clone, Debug)]
pub struct FunctionFreshen {
    pub key: FunctionKey,
    pub def_path: String,
    pub introduced: Vec<models::LifetimeAnnotation>,
    pub replaced: BTreeMap<LifetimeAnnotation, LifetimeAnnotation>,
    pub generalized: bool,
    pub edits: Vec<super::Edit>,
    pub skipped: Vec<Skipped>,
    pub bailed: Vec<models::SkipReason>,
}

#[derive(Clone, Debug)]
pub struct ImplFreshen {
    pub def_path: String,
    pub introduced: Vec<models::LifetimeAnnotation>,
    pub edits: Vec<Edit>,
    pub steps: Vec<models::Step>,
}

#[derive(Default, Debug)]
pub struct Freshened {
    pub edits: Vec<Edit>,
    pub functions: BTreeMap<FunctionKey, Freshening>,
}

pub fn freshen_annotations(
    tcx: TyCtxt<'_>,
    preserved: &BTreeSet<FunctionKey>,
    generalization: Option<&Generalization>,
) -> Freshened {
    let (funcs, impls) = collector::collect(tcx, preserved, generalization);
    let (functions, edits): (BTreeMap<_, _>, Vec<_>) = funcs
        .into_iter()
        .filter(|func| func.bailed.is_empty())
        .map(|func| {
            let freshening = Freshening {
                introduced: func.introduced,
                replaced: func.replaced,
                generalized: func.generalized,
            };
            ((func.key, freshening), func.edits)
        })
        .unzip();
    Freshened {
        edits: edits
            .into_iter()
            .chain(impls.into_iter().map(|imp| imp.edits))
            .flatten()
            .collect(),
        functions: functions
            .into_iter()
            .filter(|(_, freshening)| !freshening.is_empty())
            .collect(),
    }
}
