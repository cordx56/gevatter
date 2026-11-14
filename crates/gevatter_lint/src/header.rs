use crate::replacements;
use bridge::hir::{BodyId, ImplItemKind, ItemKind, Node, def::DefKind, def_id::LocalDefId};
use bridge::span::{FileName, Span, source_map::FileLoader};
use bridge::{FromRustc, TyCtxt};
use models::{FunctionKey, Generality, HeaderEdit, HeaderEdits, HeaderKind, Headers, ItemHeader};
use std::collections::{BTreeMap, HashMap};
use std::path::{self, PathBuf};

pub fn header_edits(
    original: &Headers,
    current: &Headers,
    generalities: &BTreeMap<FunctionKey, Generality>,
) -> HeaderEdits {
    let changed = |key: &FunctionKey| {
        original
            .get(key)
            .zip(current.get(key))
            .filter(|(from, to)| from.text != to.text)
    };
    let edits: HeaderEdits = original
        .keys()
        .filter_map(|key| {
            let (from, to) = changed(key)?;
            let edit = HeaderEdit {
                kind: from.kind,
                def_path: to.def_path.clone(),
                file: from.file.clone(),
                span: from.span,
                original: from.text.clone(),
                replaced: to.text.clone(),
                replacements: replacements(&from.text, &to.text, from.span.lo),
                parent: from
                    .parent
                    .clone()
                    .filter(|parent| changed(parent).is_some()),
                generality: generalities.get(key).copied(),
            };
            Some((key.clone(), edit))
        })
        .collect();
    tracing::info!("{} of {} headers are edited", edits.len(), original.len());
    edits
}

fn header_span(tcx: TyCtxt<'_>, def_id: LocalDefId) -> Option<(HeaderKind, Span)> {
    let function = |signature: Span, body: BodyId| {
        let body = tcx.hir_body(body).value.span;
        (
            HeaderKind::Function,
            [signature, body],
            signature.until(body),
        )
    };
    let (kind, parts, span) = match tcx.hir_node_by_def_id(def_id) {
        Node::Item(item) => match item.kind {
            ItemKind::Fn { sig, body, .. } => function(sig.span, body),
            ItemKind::Impl(implement) => {
                let self_ty = implement.self_ty.span;
                let where_clause = implement.generics.where_clause_span;
                (
                    HeaderKind::Impl,
                    [item.span, self_ty],
                    item.span.with_hi(self_ty.hi().max(where_clause.hi())),
                )
            }
            _ => return None,
        },
        Node::ImplItem(item) => match item.kind {
            ImplItemKind::Fn(sig, body) => function(sig.span, body),
            _ => return None,
        },
        _ => return None,
    };
    (!parts.iter().any(|part| part.from_expansion())).then_some((kind, span))
}

pub fn headers(tcx: TyCtxt<'_>, loader: &dyn FileLoader) -> Headers {
    let source_map = tcx.sess.source_map();
    let mut texts: HashMap<PathBuf, Option<String>> = HashMap::new();
    tcx.hir_crate_items(())
        .owners()
        .filter_map(|owner| {
            let def_id = owner.def_id;
            let (kind, span) = header_span(tcx, def_id)?;
            let source = source_map.lookup_source_file(span.lo());
            let FileName::Real(name) = &source.name else {
                return None;
            };
            let file = name.local_path()?;
            let span = models::Span::from_rustc((&*source, span));
            let text = texts
                .entry(file.to_path_buf())
                .or_insert_with(|| {
                    loader
                        .read_file(file)
                        .inspect_err(|e| tracing::warn!("failed to read {file:?}: {e:?}"))
                        .ok()
                })
                .as_deref()?
                .get(span.lo..span.hi)?
                .to_string();
            let parent = Some(tcx.local_parent(def_id))
                .filter(|parent| matches!(tcx.def_kind(*parent), DefKind::Impl { .. }))
                .map(|parent| FunctionKey::from_rustc((tcx, parent)));
            let header = ItemHeader {
                kind,
                def_path: tcx.def_path_str(def_id),
                file: path::absolute(file).unwrap_or_else(|_| file.to_path_buf()),
                span,
                text,
                parent,
            };
            Some((FunctionKey::from_rustc((tcx, def_id)), header))
        })
        .collect()
}
