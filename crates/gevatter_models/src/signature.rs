use crate::{LifetimeAnnotation, LifetimeRelation, Span};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum SignaturePosition {
    /// represents nth param; self is 0.
    Params(usize),
    Return,
}
impl SignaturePosition {
    pub fn is_param(&self) -> bool {
        matches!(self, Self::Params(_))
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum SlotSyntax {
    /// `&'a T`. Elided simply by removing the token: `&T`.
    Reference,
    /// others, typically in `Foo<'a>`.
    /// Elided by writing `'_`: `Foo<'_>`.
    Path,
}

/// Variance of a lifetime occurrence, seen from the caller of the function.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Polarity {
    Pos,
    Neg,
    /// the lifetime must match exactly if it is `&mut`.
    Inv,
}
impl Polarity {
    /// For a contravariant position; e.g. a parameter of `fn`.
    pub fn reverse(self) -> Self {
        match self {
            Self::Pos => Self::Neg,
            Self::Neg => Self::Pos,
            Self::Inv => Self::Inv,
        }
    }
}

/// One occurrence of a lifetime in the parameter types or the return type.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Slot {
    pub lifetime: LifetimeAnnotation,
    pub position: SignaturePosition,
    pub polarity: Polarity,
    pub is_self_ref: bool,
    pub syntax: SlotSyntax,
    pub span: Span,
}

/// Represents function/method's signature
#[derive(PartialEq, Eq, Default, Clone, Debug)]
pub struct Signature {
    /// lifetime annotations introduced on this function
    pub params: Vec<LifetimeAnnotation>,
    pub late_bound: Vec<LifetimeAnnotation>,
    /// lifetime annotations of the enclosing item; e.g. `impl<'a, ...>`.
    /// simplification shouldn't changes them.
    pub fixed: Vec<LifetimeAnnotation>,
    /// lifetime annotations occurrences
    pub slots: Vec<Slot>,
    /// existing predicates
    pub predicates: Vec<LifetimeRelation>,
    /// known, but not written on the function signature; e.g. where on `impl`.
    pub known: Vec<LifetimeRelation>,
}
impl Signature {
    pub fn universe(&self) -> Vec<LifetimeAnnotation> {
        self.params
            .iter()
            .chain(&self.fixed)
            .cloned()
            .chain(std::iter::once(LifetimeAnnotation::static_lifetime()))
            .collect()
    }

    pub fn slots_of(&self, lifetime: &LifetimeAnnotation) -> impl Iterator<Item = &Slot> {
        self.slots
            .iter()
            .filter(move |slot| &slot.lifetime == lifetime)
    }

    pub fn polarity(&self, lifetime: &LifetimeAnnotation) -> Polarity {
        let mut polarities = self.slots_of(lifetime).map(|slot| slot.polarity);
        match polarities.next() {
            Some(first) if polarities.all(|polarity| polarity == first) => first,
            _ => Polarity::Inv,
        }
    }

    pub fn is_constrained(&self, lifetime: &LifetimeAnnotation) -> bool {
        self.predicates
            .iter()
            .any(|relation| relation.mentions(lifetime))
    }

    pub fn replace(&mut self, from: &LifetimeAnnotation, to: &LifetimeAnnotation) {
        self.params.retain(|param| param != from);
        self.map_occurrences(|lifetime| if lifetime == from { to } else { lifetime }.clone());
    }

    pub fn rename(&mut self, names: &BTreeMap<LifetimeAnnotation, LifetimeAnnotation>) {
        let renamed =
            |lifetime: &LifetimeAnnotation| names.get(lifetime).unwrap_or(lifetime).clone();
        self.params = self.params.iter().map(renamed).collect();
        self.map_occurrences(renamed);
    }

    fn map_occurrences(&mut self, map: impl Fn(&LifetimeAnnotation) -> LifetimeAnnotation) {
        for slot in &mut self.slots {
            slot.lifetime = map(&slot.lifetime);
        }
        for relation in self.predicates.iter_mut().chain(&mut self.known) {
            relation.longer = map(&relation.longer);
            relation.shorter = map(&relation.shorter);
        }
        // a late-bound parameter mapped to `'static` or a fixed one is no longer a parameter
        let mut late_bound: Vec<_> = self
            .late_bound
            .iter()
            .map(map)
            .filter(|lifetime| !lifetime.is_static() && !self.fixed.contains(lifetime))
            .collect();
        late_bound.sort();
        late_bound.dedup();
        self.late_bound = late_bound;
    }
}

/// One rewrite applied to a signature by simplification.
/// recording rewrite edits for reporting it
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    Equate {
        removed: LifetimeAnnotation,
        into: LifetimeAnnotation,
    },
    Substitute {
        from: LifetimeAnnotation,
        to: LifetimeAnnotation,
    },
    Drop {
        lifetime: LifetimeAnnotation,
    },
    Elide {
        lifetime: LifetimeAnnotation,
    },
    Rename {
        from: LifetimeAnnotation,
        to: LifetimeAnnotation,
    },
}
