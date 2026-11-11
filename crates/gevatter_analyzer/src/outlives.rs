use models::{Region, RequiredOutlive, SkipReason};
use std::collections::{BTreeMap, BTreeSet};

/// Compute reachability of edges
#[derive(Clone, Debug)]
pub struct ReachabilityGraph {
    edges: BTreeMap<Region, BTreeSet<Region>>,
}

impl ReachabilityGraph {
    pub fn from_edges(edges: impl IntoIterator<Item = (Region, Region)>) -> Self {
        let mut graph = Self {
            edges: BTreeMap::new(),
        };
        for (from, to) in edges {
            graph.edges.entry(from).or_default().insert(to);
        }
        graph
    }

    /// Returns the reflexive transitive closure in region order.
    pub fn reachable(&self, from: Region) -> impl Iterator<Item = Region> {
        let mut visited = BTreeSet::new();
        let mut pending = vec![from];
        while let Some(region) = pending.pop() {
            if visited.insert(region) {
                if let Some(next) = self.edges.get(&region) {
                    pending.extend(next);
                }
            }
        }
        visited.into_iter()
    }
}

/// Outlive relations: Vec<(from, to)>
pub type OutliveRelations = Vec<(Region, Region)>;

/// Computes outlive requirement relations;
pub fn compute_outlive_relations(
    universals: &[Region],
    known: &[(Region, Region)],
    body_constraints: &[(Region, Region)],
) -> OutliveRelations {
    let universals: BTreeSet<_> = universals.iter().copied().collect();
    let body = ReachabilityGraph::from_edges(body_constraints.iter().copied());
    let known = ReachabilityGraph::from_edges(known.iter().copied());

    universals
        .iter()
        .flat_map(|&from| {
            let known: BTreeSet<_> = known.reachable(from).collect();
            body.reachable(from)
                .filter(|to| universals.contains(to) && !known.contains(to))
                .map(|to| (from, to))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub struct OutliveRelationReport {
    pub def_path: String,
    pub krate: String,
    pub had_diagnostics: bool,
    pub borrowck_skipped: Option<SkipReason>,
    pub required: Vec<RequiredOutlive>,
}
