use crate::{
    Simplifier,
    relation::{Outlives, transitive_reduction},
};
use models::{LifetimeAnnotation, Polarity, Signature, Step};

impl Simplifier {
    pub fn substitute(&mut self) -> &[Step] {
        let sig = &mut self.signature;
        let pinned = &self.options.pinned;
        let len_before = self.steps.len();
        loop {
            let current_len = self.steps.len();
            self.steps.extend(
                sig.params
                    .clone()
                    .into_iter()
                    .filter(|param| !pinned.contains(param))
                    .filter_map(|from| {
                        let to = target(sig, &from)?;
                        sig.replace(&from, &to);
                        Some(Step::Substitute { from, to })
                    }),
            );
            if self.steps.len() == current_len {
                break;
            }
        }
        &self.steps[len_before..]
    }

    pub fn reconstruct(&mut self) {
        let sig = &mut self.signature;
        let pinned = &self.options.pinned;
        let universe = sig.universe();
        let known = Outlives::closure(&universe, &sig.known);
        let written = Outlives::closure(&universe, &sig.predicates);
        let (kept, _) = std::mem::take(&mut sig.predicates)
            .into_iter()
            .partition::<Vec<_>, _>(|relation| {
                pinned.contains(&relation.longer) && pinned.contains(&relation.shorter)
            });
        let derived = written.relations().filter(|relation| {
            relation.longer != relation.shorter
                && !relation.longer.is_static()
                && universe.contains(&relation.shorter)
                && !known.holds(&relation.longer, &relation.shorter)
        });
        let rebuilt = transitive_reduction(&universe, derived.chain(kept.iter().cloned()))
            .into_iter()
            .filter(|relation| !kept.contains(relation))
            .collect::<Vec<_>>();
        sig.predicates = kept.into_iter().chain(rebuilt).collect();
    }
}

fn target(sig: &Signature, param: &LifetimeAnnotation) -> Option<LifetimeAnnotation> {
    let polarity = sig.polarity(param);
    if polarity == Polarity::Inv {
        return None;
    }
    let universe = sig.universe();
    let written = Outlives::closure(&universe, &sig.predicates);
    let order = Outlives::closure(&universe, sig.predicates.iter().chain(&sig.known));
    let toward =
        |order: &Outlives, from: &LifetimeAnnotation, to: &LifetimeAnnotation| match polarity {
            Polarity::Neg => order.holds(from, to),
            _ => order.holds(to, from),
        };
    let bounds: Vec<_> = universe
        .iter()
        .filter(|bound| *bound != param && toward(&order, param, bound))
        .collect();
    let mut extremes = bounds.iter().filter(|bound| {
        !bounds
            .iter()
            .any(|other| toward(&order, other, bound) && !toward(&order, bound, other))
    });
    let extreme = extremes.next().filter(|_| extremes.next().is_none())?;
    toward(&written, param, extreme).then(|| (*extreme).clone())
}
