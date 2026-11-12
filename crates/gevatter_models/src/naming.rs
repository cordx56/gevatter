//! Naming module for freshen/renaming

use crate::LifetimeAnnotation;
use std::collections::BTreeSet;

const LOWER_CASES: usize = 26;
const KEYWORDS: [&str; 5] = ["as", "do", "fn", "if", "in"];

#[derive(Default, Clone, Debug)]
pub struct LifetimeNamer {
    used: BTreeSet<LifetimeAnnotation>,
    next: usize,
}

impl LifetimeNamer {
    pub fn new(existing: impl IntoIterator<Item = LifetimeAnnotation>) -> Self {
        Self {
            used: existing.into_iter().collect(),
            next: 0,
        }
    }
    fn from_number(mut number: usize) -> LifetimeAnnotation {
        let mut name_rev = Vec::with_capacity(2);
        loop {
            let current = number % LOWER_CASES;
            name_rev.push((b'a' + current as u8) as char);
            number /= LOWER_CASES;
            if number == 0 {
                break;
            } else {
                number -= 1;
            }
        }
        LifetimeAnnotation::from(name_rev.into_iter().rev().collect::<String>())
    }
}
impl Iterator for LifetimeNamer {
    type Item = LifetimeAnnotation;
    fn next(&mut self) -> Option<Self::Item> {
        let (number, name) = (self.next..)
            .map(|number| (number, Self::from_number(number)))
            .find(|(_, name)| !KEYWORDS.contains(&name.reveal()) && !self.used.contains(name))?;
        self.next = number + 1;
        Some(name)
    }
}
