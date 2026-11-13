use crate::collectors::collect_functions;
use crate::{Edit, TargetFunc, TemporaryFileLoader, usage};
use bridge::span::source_map::FileLoader;
use bridge::{FromRustc, TyCtxt};
use models::{Freshening, FunctionKey, Simplification, SkipReason, Step};
use std::collections::{BTreeMap, BTreeSet};

mod extract;
mod rewrite;
mod variance;

use variance::Variances;

#[derive(Clone, Debug)]
pub struct Touched {
    pub freshened: BTreeMap<FunctionKey, Freshening>,
    pub fixed: BTreeSet<String>,
    pub settled: BTreeSet<String>,
    pub preserved: BTreeSet<FunctionKey>,
}

#[derive(Default, Debug)]
pub struct Simplified {
    pub edits: Vec<Edit>,
    pub functions: Vec<(String, Simplification)>,
}

pub fn simplify_signatures(tcx: TyCtxt<'_>, touched: &Touched) -> Simplified {
    if analyzer::check_well_formedness(tcx).is_some() {
        tracing::warn!("signatures are not well-formed; nothing is simplified");
        return Simplified::default();
    }
    let (functions, edits): (Vec<_>, Vec<_>) = collect_functions(tcx, &touched.preserved)
        .into_iter()
        .filter(|target| target.skipped.is_empty() && !target.is_return_opaque)
        .filter(|target| touched.settled.contains(&target.def_path))
        .filter_map(|target| {
            let untouched = Freshening::default();
            let freshening = touched
                .freshened
                .get(&FunctionKey::from_rustc((tcx, target.def_id)))
                .unwrap_or(&untouched);
            let fixed = touched.fixed.contains(&target.def_path) || freshening.generalized;
            if freshening.introduced.is_empty() && !fixed {
                return None;
            }
            let (simplification, edits) =
                match simplify_function(tcx, &target, freshening, fixed, Variances::default()) {
                    Ok((_, edits)) if edits.is_empty() => return None,
                    Ok((steps, edits)) => (Simplification::Simplified { steps }, edits),
                    Err(reason) => (
                        Simplification::Skipped {
                            reasons: vec![reason],
                        },
                        Vec::new(),
                    ),
                };
            tracing::debug!("{}: {simplification:?}", target.def_path);
            Some(((target.def_path, simplification), edits))
        })
        .unzip();
    Simplified {
        edits: edits.into_iter().flatten().collect(),
        functions,
    }
}

fn simplify_function<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: &TargetFunc<'tcx>,
    freshening: &Freshening,
    fixed: bool,
    variances: Variances,
) -> Result<(Vec<Step>, Vec<Edit>), SkipReason> {
    let extract::Extracted { signature, guarded } = extract::extract(tcx, target, variances)?;
    let usage = usage::lifetime_usage(tcx, target.def_id, target.signature.decl);
    let options = simplify::Options {
        introduced: freshening.introduced.iter().cloned().collect(),
        pinned: if fixed {
            BTreeSet::new()
        } else {
            signature
                .params
                .iter()
                .filter(|param| !freshening.introduced.contains(param))
                .cloned()
                .collect()
        },
        reserved: usage.bound_inside,
        preferred: freshening.replaced.clone(),
    };
    let simplify::SimplificationResult {
        signature: simplified,
        steps,
    } = simplify::Simplifier::new(signature.clone(), options.clone()).simplify();
    if let Some((_, reason)) = guarded
        .iter()
        .find(|(index, _)| simplified.slots[*index].lifetime.is_anonymous())
    {
        return Err(*reason);
    }
    if extract::named_lifetimes(target.generics)
        .iter()
        .any(|(id, name)| usage.in_body.contains(id) && !simplified.params.contains(name))
    {
        return Err(SkipReason::LifetimeInBody);
    }
    let text = TemporaryFileLoader
        .read_file(&target.file)
        .map_err(|_| SkipReason::ReadOnlyFile)?;
    let edits = if steps.is_empty() {
        Vec::new()
    } else {
        rewrite::rewrite(target, &text, &signature, &simplified)
    };
    Ok((steps, edits))
}
