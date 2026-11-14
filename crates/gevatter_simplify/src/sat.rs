//! This module is intended to check generality compatibility by solving SAT

use crate::relation::Outlives;
use models::{Generality, LifetimeAnnotation, Polarity, Signature};
use std::collections::BTreeMap;

type Assignment = BTreeMap<LifetimeAnnotation, LifetimeAnnotation>;

/// Check whether the signature can be instantiated to accept a call passing `actual` at its slots
pub fn sat(
    signature: &Signature,
    actual: &[LifetimeAnnotation],
    regions: &[LifetimeAnnotation],
    fixed: &Assignment,
    outlives: &Outlives,
) -> bool {
    let top = LifetimeAnnotation::static_lifetime();
    let Some(top) = regions.iter().find(|region| **region == top) else {
        return false;
    };
    let pending: Vec<_> = signature
        .params
        .iter()
        .map(|param| (param.clone(), regions.to_vec()))
        .chain(
            signature
                .fixed
                .iter()
                .map(|name| (name.clone(), fixed.get(name).into_iter().cloned().collect())),
        )
        .chain(std::iter::once((top.clone(), vec![Clone::clone(top)])))
        .map(|(variable, candidates)| {
            let allowed = candidates
                .into_iter()
                .filter(|chosen| {
                    signature
                        .slots
                        .iter()
                        .zip(actual)
                        .filter(|(slot, _)| slot.lifetime == variable)
                        .all(|(slot, actual)| accepts(slot.polarity, chosen, actual, outlives))
                })
                .collect::<Vec<_>>();
            (variable, allowed)
        })
        .collect();
    signature.slots.len() == actual.len()
        && signature.slots.iter().all(|slot| {
            pending
                .iter()
                .any(|(variable, _)| *variable == slot.lifetime)
        })
        && search(signature, &pending, &mut Assignment::new(), outlives)
}

/// Check whether two function signatures' generalities are comparable
pub fn is_comparable(original: &Signature, new: &Signature) -> bool {
    original.fixed.len() == new.fixed.len()
        && original.slots.len() == new.slots.len()
        && original
            .slots
            .iter()
            .zip(&new.slots)
            .all(|(from, to)| from.position == to.position && from.polarity == to.polarity)
}

/// Check whether `new` accepts every call that `original` accepts
pub fn generalizes(new: &Signature, original: &Signature) -> bool {
    let regions = original.universe();
    let outlives = Outlives::closure(&regions, original.predicates.iter().chain(&original.known));
    let actual: Vec<_> = original
        .slots
        .iter()
        .map(|slot| slot.lifetime.clone())
        .collect();
    let fixed = new
        .fixed
        .clone()
        .into_iter()
        .zip(original.fixed.clone())
        .collect();
    is_comparable(original, new) && sat(new, &actual, &regions, &fixed, &outlives)
}

/// Compare the generality of `new` against `original`; `None` if they are not comparable
pub fn classify(original: &Signature, new: &Signature) -> Option<Generality> {
    is_comparable(original, new).then(|| {
        match (generalizes(new, original), generalizes(original, new)) {
            (true, true) => Generality::Equivalent,
            (true, false) => Generality::MoreGeneral,
            (false, true) => Generality::LessGeneral,
            (false, false) => Generality::Incomparable,
        }
    })
}

fn accepts(
    polarity: Polarity,
    chosen: &LifetimeAnnotation,
    actual: &LifetimeAnnotation,
    outlives: &Outlives,
) -> bool {
    match polarity {
        Polarity::Neg => outlives.holds(actual, chosen),
        Polarity::Pos => outlives.holds(chosen, actual),
        Polarity::Inv => outlives.equivalent(chosen, actual),
    }
}

fn search(
    signature: &Signature,
    pending: &[(LifetimeAnnotation, Vec<LifetimeAnnotation>)],
    assignment: &mut Assignment,
    outlives: &Outlives,
) -> bool {
    let Some(((variable, candidates), rest)) = pending.split_first() else {
        return signature
            .predicates
            .iter()
            .chain(&signature.known)
            .all(|relation| {
                assignment
                    .get(&relation.longer)
                    .zip(assignment.get(&relation.shorter))
                    .is_some_and(|(longer, shorter)| outlives.holds(longer, shorter))
            });
    };
    candidates.iter().any(|candidate| {
        assignment.insert(variable.clone(), candidate.clone());
        let found = search(signature, rest, assignment, outlives);
        assignment.remove(variable);
        found
    })
}
