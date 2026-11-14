use models::{LifetimeAnnotation, Signature, Step};
use std::collections::{BTreeMap, BTreeSet};

pub mod contract;
pub mod elision;
pub mod relation;
pub mod rename;
pub mod restore;
pub mod sat;
pub mod substitute;

#[derive(Default, Clone, Debug)]
pub struct Options {
    pub introduced: BTreeSet<LifetimeAnnotation>,
    pub pinned: BTreeSet<LifetimeAnnotation>,
    pub reserved: BTreeSet<LifetimeAnnotation>,
    pub preferred: BTreeMap<LifetimeAnnotation, LifetimeAnnotation>,
}

#[derive(Default, Clone, Debug)]
pub struct SimplificationResult {
    pub signature: Signature,
    pub steps: Vec<Step>,
}

#[derive(Default, Clone, Debug)]
pub struct Simplifier {
    signature: Signature,
    options: Options,
    steps: Vec<Step>,
}
impl Simplifier {
    pub fn new(signature: Signature, options: Options) -> Self {
        Self {
            signature,
            options,
            steps: Vec::new(),
        }
    }

    /// an entry point of simplification process
    pub fn simplify(mut self) -> SimplificationResult {
        self.contract();
        self.substitute();
        self.reconstruct();
        self.restore();
        self.drop_unused();
        self.elide();
        self.rename();

        SimplificationResult {
            signature: self.signature,
            steps: self.steps,
        }
    }

    pub fn drop_unused(&mut self) -> &[Step] {
        let sig = &mut self.signature;
        let pinned = &self.options.pinned;

        let (used, unused) = std::mem::take(&mut sig.params)
            .into_iter()
            .partition(|param| {
                pinned.contains(param)
                    || sig.slots_of(param).next().is_some()
                    || sig.is_constrained(param)
            });
        sig.params = used;

        let universe = sig.universe();
        sig.known.retain(|relation| {
            universe.contains(&relation.longer) && universe.contains(&relation.shorter)
        });
        sig.late_bound
            .retain(|lifetime| universe.contains(lifetime));

        let before_len = self.steps.len();
        self.steps
            .extend(unused.into_iter().map(|lifetime| Step::Drop { lifetime }));
        &self.steps[before_len..]
    }
}
