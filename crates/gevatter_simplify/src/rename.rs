use crate::Simplifier;
use models::{LifetimeNamer, Step};

impl Simplifier {
    pub fn rename(&mut self) -> &[Step] {
        let sig = &mut self.signature;
        let (renamed, kept) = sig
            .params
            .iter()
            .cloned()
            .partition::<Vec<_>, _>(|param| self.options.introduced.contains(param));
        let namer = LifetimeNamer::new(
            kept.iter()
                .chain(&sig.fixed)
                .chain(&self.options.reserved)
                .cloned(),
        );

        let names: Vec<_> = renamed
            .into_iter()
            .zip(namer)
            .filter(|(from, to)| from != to)
            .collect();
        sig.rename(&names.iter().cloned().collect());
        let len_before = self.steps.len();
        self.steps.extend(
            names
                .into_iter()
                .map(|(from, to)| Step::Rename { from, to }),
        );
        &self.steps[len_before..]
    }
}
