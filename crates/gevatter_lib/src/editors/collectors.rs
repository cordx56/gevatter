use super::TargetFunc;
use crate::{
    Ident, TyCtxt,
    hir::{
        self, AmbigArg, Block, GenericParam, GenericParamKind, ImplItemKind, ItemKind, Node,
        TraitItemKind, Ty, TyKind,
        def::DefKind,
        intravisit::{self, Visitor},
    },
    middle,
    span::FileName,
};
use models::SkipReason;
use std::path::PathBuf;

pub fn collect_functions<'tcx>(tcx: TyCtxt<'tcx>) -> Vec<TargetFunc> {
    let mut target_funcs = Vec::new();

    for owner in tcx.hir_crate_items(()).owners() {
        let def_id = owner.def_id;
        tracing::trace!("collecting function {def_id:?}");

        // TODO: comment
        if !matches!(tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn) {
            tracing::trace!("DefKind is not Fn or AssocFn; skip");
            continue;
        }

        // TODO: comment
        let (signature, generics, ident) = match tcx.hir_node_by_def_id(def_id) {
            Node::Item(item) => match item.kind {
                ItemKind::Fn {
                    sig,
                    ident,
                    generics,
                    ..
                } => (sig, generics, ident),
                _ => continue,
            },
            Node::ImplItem(item) => match item.kind {
                ImplItemKind::Fn(sig, _) => (sig, item.generics, item.ident),
                _ => continue,
            },
            Node::TraitItem(item) => match item.kind {
                TraitItemKind::Fn(sig, _) => (sig, item.generics, item.ident),
                _ => continue,
            },
            _ => continue,
        };

        let mut skipped = Vec::new();
        let parent = tcx.local_parent(def_id);
        tracing::trace!("parent of {def_id:?} is {parent:?}");
        let parent_generics = match tcx.hir_node_by_def_id(parent) {
            Node::Item(item) => match item.kind {
                ItemKind::Impl(item) => {
                    if item.of_trait.is_some() {
                        skipped.push(SkipReason::TraitItem);
                    }
                    Some(item.generics)
                }
                ItemKind::Trait { .. } => {
                    skipped.push(SkipReason::TraitItem);
                    None
                }
                _ => None,
            },
            _ => None,
        };
        tracing::trace!("parent has generics: {}", parent_generics.is_some());

        if generics
            .params
            .iter()
            .chain(
                parent_generics
                    .into_iter()
                    .flat_map(|generics| generics.params),
            )
            .any(|params| !matches!(params.kind, GenericParamKind::Lifetime { .. }))
        {
            skipped.push(SkipReason::TypeParameters);
        }

        // TODO: we do not treat async fn, future work
        if signature.header.is_async() {
            skipped.push(SkipReason::AsyncFn);
        }

        // skip expanded code (e.g. macro)
        let mut expansion_check = SignatureExpansionCheck::default();
        expansion_check.visit_fn_ret_ty(&signature.decl.output);
        let is_return_opaque = expansion_check.opaque;
        expansion_check.visit_fn_decl(signature.decl);
        expansion_check.visit_generics(generics);
        if expansion_check.expanded
            || signature.span.from_expansion()
            || ident.span.from_expansion()
            || generics.span.from_expansion()
        {
            skipped.push(SkipReason::MacroExpanded);
        }

        let source = tcx
            .sess
            .source_map()
            .lookup_source_file(signature.span.lo());
        let file = match &source.name {
            FileName::Real(name) => name.local_path().map(PathBuf::from),
            _ => None,
        };
        if file.is_none() {
            skipped.push(SkipReason::Unknown);
        }
        let mut unsafe_check = UnsafeCheck {
            tcx,
            exists: signature.header.is_unsafe(),
        };
        if unsafe_check.exists {
            tracing::trace!("signature header is unsafe");
        }
        if let Some(body) = tcx.hir_maybe_body_owned_by(def_id) {
            unsafe_check.visit_body(body);
        }
        if unsafe_check.exists {
            skipped.push(SkipReason::UnsafeBody);
        }

        // TODO: push
        tracing::trace!("{def_id:?} is target; register");
        target_funcs.push(TargetFunc {
            file: file.unwrap_or_default(),
            source,
            def_id,
            def_path: tcx.def_path_str(def_id.to_def_id()),
            skipped,
            ident,
            signature,
            generics,
            parent_generics,
            is_return_opaque,
        });
    }
    target_funcs.sort_by(|a, b| a.def_path.cmp(&b.def_path));
    target_funcs
}

#[derive(Default, Clone, Copy, Debug)]
struct SignatureExpansionCheck {
    expanded: bool,
    opaque: bool,
}
impl<'hir> Visitor<'hir> for SignatureExpansionCheck {
    fn visit_ty(&mut self, ty: &'hir Ty<'hir, AmbigArg>) -> Self::Result {
        self.expanded |= ty.span.from_expansion();
        self.opaque |= matches!(ty.kind, TyKind::OpaqueDef(_));
        intravisit::walk_ty(self, ty)
    }
    fn visit_ident(&mut self, ident: Ident) -> Self::Result {
        self.expanded |= ident.span.from_expansion();
        intravisit::walk_ident(self, ident)
    }
    fn visit_generic_param(&mut self, param: &'hir GenericParam<'hir>) -> Self::Result {
        self.expanded |= param.span.from_expansion();
        intravisit::walk_generic_param(self, param)
    }
}

struct UnsafeCheck<'tcx> {
    tcx: TyCtxt<'tcx>,
    exists: bool,
}
impl<'hir> Visitor<'hir> for UnsafeCheck<'hir> {
    type NestedFilter = middle::hir::nested_filter::OnlyBodies;
    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }
    fn visit_block(&mut self, block: &'hir Block<'hir>) -> Self::Result {
        if !block.span.from_expansion()
            && matches!(block.rules, hir::BlockCheckMode::UnsafeBlock(_))
        {
            tracing::trace!("found unsafe block");
            self.exists = true;
        }
        intravisit::walk_block(self, block)
    }
}
