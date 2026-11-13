use crate::{Options, Simplifier};
use models::{LifetimeAnnotation, Signature, SignaturePosition, Slot, Step};
use std::collections::{BTreeMap, BTreeSet};

impl Simplifier {
    pub fn elide(&mut self) -> &[Step] {
        let sig = &mut self.signature;
        let before_length = self.steps.len();
        let Some((candidate, hidden)) = accepted_candidate(sig, &self.options) else {
            return &[];
        };
        *sig = candidate;
        self.steps
            .extend(hidden.into_iter().map(|lifetime| Step::Elide { lifetime }));
        &self.steps[before_length..]
    }
}

fn is_anonymous_output(slot: &Slot) -> bool {
    slot.position == SignaturePosition::Return && slot.lifetime.is_anonymous()
}

pub fn resolve_elision(signature: &Signature) -> Option<Signature> {
    let mut resolved = signature.clone();
    let mut fresh = (0..)
        .map(|serial| LifetimeAnnotation::from(format!("elided{serial}")))
        .filter(|name| !signature.params.contains(name) && !signature.fixed.contains(name));
    for slot in resolved
        .slots
        .iter_mut()
        .filter(|slot| slot.position.is_param() && slot.lifetime.is_anonymous())
    {
        slot.lifetime = fresh.next()?;
        resolved.params.push(slot.lifetime.clone());
    }
    if resolved.slots.iter().any(is_anonymous_output) {
        let inputs: Vec<_> = resolved
            .slots
            .iter()
            .filter(|slot| slot.position.is_param())
            .collect();
        let source = inputs
            .iter()
            .find(|slot| slot.is_self_ref)
            .or(match inputs.as_slice() {
                [only] => Some(only),
                _ => None,
            })?
            .lifetime
            .clone();
        resolved
            .slots
            .iter_mut()
            .filter(|slot| is_anonymous_output(slot))
            .for_each(|slot| slot.lifetime = source.clone());
    }
    Some(resolved)
}

type Renaming<'a> = BTreeMap<&'a LifetimeAnnotation, &'a LifetimeAnnotation>;

pub fn alpha_equivalent(a: &Signature, b: &Signature) -> bool {
    if a.fixed != b.fixed || a.params.len() != b.params.len() || a.slots.len() != b.slots.len() {
        return false;
    }
    let top = LifetimeAnnotation::static_lifetime();
    let mut renaming: Renaming = a.fixed.iter().map(|fixed| (fixed, fixed)).collect();
    renaming.insert(&top, &top);
    let slots_match = a.slots.iter().zip(&b.slots).all(|(x, y)| {
        x.position == y.position
            && x.polarity == y.polarity
            && x.is_self_ref == y.is_self_ref
            && x.syntax == y.syntax
            && bind(&mut renaming, &x.lifetime, &y.lifetime)
    });
    slots_match && complete(a, b, renaming)
}

fn bind<'a>(
    renaming: &mut Renaming<'a>,
    from: &'a LifetimeAnnotation,
    to: &'a LifetimeAnnotation,
) -> bool {
    match renaming.get(from) {
        Some(bound) => *bound == to,
        None => {
            let free =
                from.is_static() == to.is_static() && !renaming.values().any(|bound| *bound == to);
            if free {
                renaming.insert(from, to);
            }
            free
        }
    }
}

fn complete<'a>(a: &'a Signature, b: &'a Signature, renaming: Renaming<'a>) -> bool {
    match a.params.iter().find(|param| !renaming.contains_key(param)) {
        Some(param) => b
            .params
            .iter()
            .filter(|candidate| !renaming.values().any(|bound| bound == candidate))
            .any(|candidate| {
                let mut extended = renaming.clone();
                extended.insert(param, candidate);
                complete(a, b, extended)
            }),
        None => {
            let renamed: Option<BTreeSet<_>> = a
                .predicates
                .iter()
                .map(|relation| {
                    renaming
                        .get(&relation.longer)
                        .copied()
                        .zip(renaming.get(&relation.shorter).copied())
                })
                .collect();
            let expected: BTreeSet<_> = b
                .predicates
                .iter()
                .map(|relation| (&relation.longer, &relation.shorter))
                .collect();
            renamed == Some(expected)
        }
    }
}

fn accepted_candidate(
    signature: &Signature,
    options: &Options,
) -> Option<(Signature, Vec<LifetimeAnnotation>)> {
    let free = |lifetime: &LifetimeAnnotation| {
        options.introduced.contains(lifetime) && !signature.is_constrained(lifetime)
    };
    let singles: Vec<_> = signature
        .params
        .iter()
        .filter(|param| {
            let mut slots = signature.slots_of(param);
            free(param)
                && slots.next().is_some_and(|slot| slot.position.is_param())
                && slots.next().is_none()
        })
        .collect();
    let mut outputs = signature
        .slots
        .iter()
        .filter(|slot| slot.position == SignaturePosition::Return)
        .map(|slot| &slot.lifetime);
    let output = outputs
        .next()
        .filter(|first| free(first) && !options.preferred.contains_key(*first))
        .filter(|first| outputs.all(|other| other == *first));
    [output, None].into_iter().find_map(|output| {
        let hidden: Vec<_> = signature
            .params
            .iter()
            .filter(|param| singles.contains(param) || output == Some(*param))
            .cloned()
            .collect();
        let candidate = hide(signature, &hidden);
        resolve_elision(&candidate)
            .is_some_and(|resolved| alpha_equivalent(signature, &resolved))
            .then_some((candidate, hidden))
    })
}

fn hide(signature: &Signature, hidden: &[LifetimeAnnotation]) -> Signature {
    let mut candidate = signature.clone();
    candidate.params.retain(|param| !hidden.contains(param));
    candidate
        .late_bound
        .retain(|lifetime| !hidden.contains(lifetime));
    candidate.known.retain(|relation| {
        !hidden.contains(&relation.longer) && !hidden.contains(&relation.shorter)
    });
    for slot in &mut candidate.slots {
        if hidden.contains(&slot.lifetime) {
            slot.lifetime = LifetimeAnnotation::anonymous();
        }
    }
    candidate
}
