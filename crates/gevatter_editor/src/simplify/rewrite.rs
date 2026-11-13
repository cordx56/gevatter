use crate::text::{with_leading_blank, with_trailing_whitespace};
use crate::{Edit, TargetFunc};
use bridge::FromRustc;
use bridge::hir;
use models::{LifetimeAnnotation, LifetimeRelation, Signature, SlotSyntax, Span};
use std::collections::BTreeSet;

pub fn rewrite(
    target: &TargetFunc<'_>,
    text: &str,
    before: &Signature,
    after: &Signature,
) -> Vec<Edit> {
    let offsets = |span| Span::from_rustc((&*target.source, span));
    let slots = before
        .slots
        .iter()
        .zip(&after.slots)
        .filter(|(old, new)| old.lifetime != new.lifetime)
        .map(|(old, new)| {
            if new.lifetime.is_anonymous() && new.syntax == SlotSyntax::Reference {
                (with_trailing_whitespace(text, old.span), String::new())
            } else {
                (old.span, new.lifetime.to_string())
            }
        });
    let relations = |signature: &Signature| -> BTreeSet<LifetimeRelation> {
        signature.predicates.iter().cloned().collect()
    };
    let rebound = relations(before) != relations(after);
    let bounded = |name: &LifetimeAnnotation| {
        target
            .generics
            .predicates
            .iter()
            .filter(|predicate| !predicate.kind.in_where_clause())
            .filter_map(move |predicate| match predicate.kind {
                hir::WherePredicateKind::RegionPredicate(region)
                    if region.lifetime.ident.name.as_str() == name.to_string() =>
                {
                    Some(offsets(predicate.span).hi)
                }
                _ => None,
            })
            .max()
    };
    let inline: Vec<_> = target
        .generics
        .params
        .iter()
        .filter_map(|param| {
            let name = LifetimeAnnotation::from(param.name.ident().name.as_str());
            let hi = bounded(&name)?;
            let lo = offsets(param.span).lo;
            Some((name, Span { lo, hi }))
        })
        .collect();
    let declaration = |param: &LifetimeAnnotation| {
        inline
            .iter()
            .find(|(name, _)| !rebound && name == param)
            .and_then(|(_, span)| text.get(span.lo..span.hi))
            .map_or_else(|| param.to_string(), str::to_owned)
    };
    let parameters =
        (before.params != after.params || (rebound && !inline.is_empty())).then(|| {
            let declarations: Vec<_> = after.params.iter().map(declaration).collect();
            let list = if declarations.is_empty() {
                String::new()
            } else {
                format!("<{}>", declarations.join(", "))
            };
            (offsets(target.generics.span), list)
        });
    let clause = offsets(target.generics.where_clause_span);
    let predicates = render(after);
    let has_clause = target
        .generics
        .predicates
        .iter()
        .any(|predicate| predicate.kind.in_where_clause());
    let bounds = match (has_clause, predicates.is_empty()) {
        _ if !rebound => None,
        (false, true) => None,
        (false, false) if clause.lo == clause.hi => Some((clause, format!(" where {predicates}"))),
        (false, false) => Some((
            Span {
                lo: clause.hi,
                hi: clause.hi,
            },
            format!(" {predicates}"),
        )),
        (true, true) => Some((with_leading_blank(text, clause), String::new())),
        (true, false) => Some((clause, format!("where {predicates}"))),
    };
    slots
        .chain(parameters)
        .chain(bounds)
        .filter(|(span, replace_with)| text.get(span.lo..span.hi) != Some(replace_with.as_str()))
        .map(|(span, replace_with)| Edit {
            krate: target.krate.clone(),
            file: target.file.clone(),
            span,
            replace_with,
        })
        .collect()
}

fn render(signature: &Signature) -> String {
    let order: Vec<_> = [LifetimeAnnotation::static_lifetime()]
        .into_iter()
        .chain(signature.fixed.iter().cloned())
        .chain(signature.params.iter().cloned())
        .collect();
    let outlived = |longer: &LifetimeAnnotation| -> Vec<String> {
        order
            .iter()
            .filter(|shorter| {
                signature.predicates.contains(&LifetimeRelation {
                    longer: longer.clone(),
                    shorter: (*shorter).clone(),
                })
            })
            .map(ToString::to_string)
            .collect()
    };
    order
        .iter()
        .map(|longer| (longer, outlived(longer)))
        .filter(|(_, shorter)| !shorter.is_empty())
        .map(|(longer, shorter)| format!("{longer}: {}", shorter.join(" + ")))
        .collect::<Vec<_>>()
        .join(", ")
}
