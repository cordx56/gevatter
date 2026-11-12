use crate::{Simplifier, relation::Outlives};
use models::{LifetimeAnnotation, Step};

impl Simplifier {
    pub fn contract(&mut self) -> &[Step] {
        let sig = &mut self.signature;
        let pinned = &self.options.pinned;
        let order = Outlives::closure(&sig.universe(), sig.predicates.iter().chain(&sig.known));
        let representatives: Vec<_> = [LifetimeAnnotation::static_lifetime()]
            .into_iter()
            .chain(sig.fixed.iter().cloned())
            .chain(sig.params.iter().cloned())
            .collect();
        let len_before = self.steps.len();
        self.steps.extend(
            sig.params
                .clone()
                .into_iter()
                .filter(|param| !pinned.contains(param))
                .filter_map(|removed| {
                    let into = representatives
                        .iter()
                        .find(|candidate| order.equivalent(&removed, candidate))
                        .filter(|representative| **representative != removed)?
                        .clone();
                    sig.replace(&removed, &into);
                    Some(Step::Equate { removed, into })
                }),
        );
        &self.steps[len_before..]
    }
}
