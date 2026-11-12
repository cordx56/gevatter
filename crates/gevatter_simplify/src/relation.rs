use models::{LifetimeAnnotation, LifetimeRelation};
use std::collections::{BTreeMap, BTreeSet};

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Outlives(BTreeMap<LifetimeAnnotation, BTreeSet<LifetimeAnnotation>>);

impl Outlives {
    pub fn closure<'a>(
        universe: &[LifetimeAnnotation],
        relations: impl IntoIterator<Item = &'a LifetimeRelation>,
    ) -> Self {
        let top = LifetimeAnnotation::static_lifetime();
        let mut direct: BTreeMap<&LifetimeAnnotation, BTreeSet<&LifetimeAnnotation>> =
            BTreeMap::new();
        for relation in relations {
            direct
                .entry(&relation.longer)
                .or_default()
                .insert(&relation.shorter);
        }
        direct.entry(&top).or_default().extend(universe);
        Self(
            universe
                .iter()
                .map(|longer| (longer.clone(), reachable(&direct, longer)))
                .collect(),
        )
    }

    pub fn holds(&self, longer: &LifetimeAnnotation, shorter: &LifetimeAnnotation) -> bool {
        self.0
            .get(longer)
            .is_some_and(|reachable| reachable.contains(shorter))
    }

    pub fn equivalent(&self, a: &LifetimeAnnotation, b: &LifetimeAnnotation) -> bool {
        self.holds(a, b) && self.holds(b, a)
    }

    pub fn relations(&self) -> impl Iterator<Item = LifetimeRelation> {
        self.0.iter().flat_map(|(longer, reachable)| {
            reachable.iter().map(|shorter| LifetimeRelation {
                longer: longer.clone(),
                shorter: shorter.clone(),
            })
        })
    }
}

fn reachable(
    direct: &BTreeMap<&LifetimeAnnotation, BTreeSet<&LifetimeAnnotation>>,
    from: &LifetimeAnnotation,
) -> BTreeSet<LifetimeAnnotation> {
    let mut visited = BTreeSet::new();
    let mut pending = vec![from];
    while let Some(lifetime) = pending.pop() {
        if visited.insert(lifetime.clone()) {
            pending.extend(direct.get(lifetime).into_iter().flatten());
        }
    }
    visited
}

pub fn transitive_reduction(
    universe: &[LifetimeAnnotation],
    relations: impl IntoIterator<Item = LifetimeRelation>,
) -> Vec<LifetimeRelation> {
    let relations: BTreeSet<_> = relations.into_iter().collect();
    relations
        .iter()
        .fold(relations.clone(), |mut reduced, relation| {
            reduced.remove(relation);
            if !Outlives::closure(universe, &reduced).holds(&relation.longer, &relation.shorter) {
                reduced.insert(relation.clone());
            }
            reduced
        })
        .into_iter()
        .collect()
}
