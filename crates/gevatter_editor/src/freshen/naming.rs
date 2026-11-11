//! # Lifetime naming module
//!
//! Intended to generate fresh lifetime names

use models::LifetimeAnnotation;
use std::collections::BTreeSet;

const LOWER_CASES: usize = 26;

#[derive(Debug)]
pub struct LifetimeNamer {
    used: BTreeSet<String>,
    next: usize,
}

impl LifetimeNamer {
    pub fn new(existing: impl Iterator<Item = String>) -> Self {
        Self {
            used: existing.collect(),
            next: 0,
        }
    }
    pub fn fresh(&mut self) -> LifetimeAnnotation {
        loop {
            let mut name_rev = Vec::new();
            let mut next = self.next;
            loop {
                let current = next % LOWER_CASES;
                name_rev.push((b'a' + current as u8) as char);
                next /= LOWER_CASES;
                if next == 0 {
                    break;
                } else {
                    next -= 1;
                }
            }
            self.next += 1;
            let name: String = name_rev.into_iter().rev().collect();
            if self.used.insert(format!("'{name}")) {
                return LifetimeAnnotation::from(name);
            }
        }
    }
}
