use crate::Simplifier;
use models::{LifetimeAnnotation, Signature, Step};

fn is_single_input(signature: &Signature, lifetime: &LifetimeAnnotation) -> bool {
    let mut slots = signature.slots_of(lifetime);
    slots.next().is_some_and(|slot| slot.position.is_param()) && slots.next().is_none()
}

fn is_vacant(signature: &Signature, lifetime: &LifetimeAnnotation) -> bool {
    signature.params.contains(lifetime)
        && signature.slots_of(lifetime).next().is_none()
        && !signature.is_constrained(lifetime)
}

impl Simplifier {
    pub fn restore(&mut self) -> &[Step] {
        let sig = &mut self.signature;
        let preferred = &self.options.preferred;
        let len_before = self.steps.len();
        self.steps
            .extend(sig.params.clone().into_iter().filter_map(|from| {
                let to = preferred.get(&from)?.clone();
                let elidable = is_single_input(sig, &from) && !sig.is_constrained(&from);
                (!elidable && is_vacant(sig, &to)).then(|| {
                    sig.replace(&from, &to);
                    Step::Rename { from, to }
                })
            }));
        &self.steps[len_before..]
    }
}
