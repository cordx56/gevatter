use bridge::errors::{Applicability, DiagDecorator};
use bridge::hir::{HirId, def::DefKind, def_id::LocalDefId};
use bridge::interface::passes::DEFAULT_QUERY_PROVIDERS;
use bridge::lint::{Lint, LintStore};
use bridge::middle::util::Providers;
use bridge::session::Session;
use bridge::span::{BytePos, Ident, SourceFile, Span};
use bridge::{FromRustc, TyCtxt};
use models::{FunctionKey, Generality, HeaderEdit, HeaderEdits, HeaderKind};
use std::collections::BTreeMap;

mod fixable;
mod permissive;

pub use fixable::*;
pub use permissive::*;

/// Gevatter has to be registered as crate tool
pub fn override_queries(_: &Session, providers: &mut Providers) {
    providers.queries.registered_lint_tools = |tcx, ()| {
        let mut tools = (DEFAULT_QUERY_PROVIDERS.queries.registered_lint_tools)(tcx, ());
        tools.insert(Ident::from_str("gevatter"));
        tools
    };
}

pub fn register_lints(store: &mut LintStore) {
    store.register_lints(&[FIXABLE_LIFETIME_ERRORS, NON_PERMISSIVE_LIFETIMES]);
}

/// used for emitting lint by [`SignatureLints::emit`]
pub trait SignatureLint {
    const LINT: &'static Lint;
    const HELP: &'static str;

    /// generate lint message using function name
    fn message(function: &str) -> String;
}

type Parts = Vec<(Span, String)>;

fn parts(source: &SourceFile, edit: &HeaderEdit) -> Parts {
    edit.replacements
        .iter()
        .map(|replacement| {
            (
                span_in(source, replacement.span),
                replacement.replace_with.clone(),
            )
        })
        .collect()
}

fn span_in(source: &SourceFile, span: models::Span) -> Span {
    let position = |offset: usize| {
        u32::try_from(offset).map_or(BytePos(u32::MAX), |offset| {
            source.normalized_byte_pos(offset)
        })
    };
    Span::with_root_ctxt(position(span.lo), position(span.hi))
}

fn is_current(tcx: TyCtxt<'_>, source: &SourceFile, edit: &HeaderEdit) -> bool {
    tcx.sess
        .source_map()
        .span_to_snippet(span_in(source, edit.span))
        // we have to replace CRLF with LF; rustc ignores CR
        .is_ok_and(|text| text == edit.original.replace("\r\n", "\n"))
}

#[derive(Clone, Debug)]
struct SignatureFix {
    def_id: LocalDefId,
    generality: Option<Generality>,
    parts: Parts,
    parent: Option<(FunctionKey, Parts)>,
}
impl SignatureFix {
    fn new(tcx: TyCtxt<'_>, def_id: LocalDefId, edits: &HeaderEdits) -> Option<Self> {
        let edit = edits
            .get(&FunctionKey::from_rustc((tcx, def_id)))
            .filter(|edit| edit.kind == HeaderKind::Function)?;
        let parent = match &edit.parent {
            Some(parent) => Some((parent, edits.get(parent)?)),
            None => None,
        };
        let source = tcx
            .sess
            .source_map()
            .lookup_source_file(tcx.def_span(def_id).lo());
        let is_current = std::iter::once(edit)
            .chain(parent.map(|(_, edit)| edit))
            .all(|edit| is_current(tcx, &source, edit));
        if !is_current {
            tracing::warn!(
                "the source of {} differs from the edited one",
                edit.def_path
            );
            return None;
        }
        Some(Self {
            def_id,
            generality: edit.generality,
            parts: parts(&source, edit),
            parent: parent.map(|(key, edit)| (key.clone(), parts(&source, edit))),
        })
    }

    fn hir_id(&self, tcx: TyCtxt<'_>) -> HirId {
        tcx.local_def_id_to_hir_id(self.def_id)
    }

    fn emit<L: SignatureLint>(self, tcx: TyCtxt<'_>, with_parent: bool) {
        let hir_id = self.hir_id(tcx);
        let Self {
            def_id,
            parts,
            parent,
            ..
        } = self;
        let parts: Parts = parent
            .filter(|_| with_parent)
            .into_iter()
            .flat_map(|(_, parts)| parts)
            .chain(parts)
            .collect();
        let span = tcx
            .def_ident_span(def_id)
            .unwrap_or_else(|| tcx.def_span(def_id));
        let message = L::message(&tcx.def_path_str(def_id));
        tcx.emit_node_span_lint(
            L::LINT,
            hir_id,
            span,
            DiagDecorator(|diag| {
                diag.primary_message(message);
                diag.multipart_suggestion(L::HELP, parts, Applicability::MachineApplicable);
            }),
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LintKind {
    FixableLifetimeErrors,
    NonPermissiveLifetimes,
}
impl LintKind {
    fn lint(self) -> &'static Lint {
        match self {
            Self::FixableLifetimeErrors => FixableLifetimeErrors::LINT,
            Self::NonPermissiveLifetimes => NonPermissiveLifetimes::LINT,
        }
    }

    fn of(tcx: TyCtxt<'_>, fix: &SignatureFix) -> Option<Self> {
        match fix.generality {
            _ if FixableLifetimeErrors::has_errors(tcx, fix.def_id) => {
                Some(Self::FixableLifetimeErrors)
            }
            Some(Generality::Equivalent) => None,
            Some(Generality::LessGeneral | Generality::Incomparable) => {
                Some(Self::FixableLifetimeErrors)
            }
            Some(Generality::MoreGeneral) | None => Some(Self::NonPermissiveLifetimes),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SignatureLints(Vec<(LintKind, SignatureFix)>);
impl SignatureLints {
    pub fn new(tcx: TyCtxt<'_>, edits: &HeaderEdits) -> Self {
        bridge::check_well_formedness(tcx);
        let lints = Self(
            tcx.hir_crate_items(())
                .owners()
                .map(|owner| owner.def_id)
                .filter(|def_id| matches!(tcx.def_kind(*def_id), DefKind::Fn | DefKind::AssocFn))
                .filter_map(|def_id| SignatureFix::new(tcx, def_id, edits))
                .filter_map(|fix| Some((LintKind::of(tcx, &fix)?, fix)))
                .collect(),
        );
        if lints.leaves_errors(tcx) {
            lints.promoted()
        } else {
            lints
        }
    }

    /// Check whether errors are remain or not
    pub fn leaves_errors(&self, tcx: TyCtxt<'_>) -> bool {
        let is_fixed = |def_id: LocalDefId| {
            self.0
                .iter()
                .any(|(kind, fix)| *kind == LintKind::FixableLifetimeErrors && fix.def_id == def_id)
        };
        tcx.hir_body_owners()
            .filter(|def_id| !tcx.is_typeck_child(def_id.to_def_id()))
            .filter(|def_id| !tcx.is_trivial_const(*def_id))
            .filter(|def_id| !is_fixed(*def_id))
            .any(|def_id| tcx.mir_borrowck(def_id).is_err())
    }

    fn promoted(self) -> Self {
        Self(
            self.0
                .into_iter()
                .map(|(_, fix)| (LintKind::FixableLifetimeErrors, fix))
                .collect(),
        )
    }

    /// has fixable errors?
    pub fn fixes_errors(&self) -> bool {
        self.0
            .iter()
            .any(|(kind, _)| *kind == LintKind::FixableLifetimeErrors)
    }

    pub fn emit(self, tcx: TyCtxt<'_>) {
        // each impl header edit is carried by only the first non-allowed method lint;
        // `cargo fix` drops whole suggestions that share an identical replacement
        let carriers: BTreeMap<_, _> = self
            .0
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, (kind, fix))| {
                !tcx.lint_level_spec_at_node(kind.lint(), fix.hir_id(tcx))
                    .is_allow()
            })
            .filter_map(|(index, (_, fix))| Some((fix.parent.as_ref()?.0.clone(), index)))
            .collect();
        self.0
            .into_iter()
            .enumerate()
            .for_each(|(index, (kind, fix))| {
                let with_parent = fix
                    .parent
                    .as_ref()
                    .is_some_and(|(parent, _)| carriers.get(parent) == Some(&index));
                match kind {
                    LintKind::FixableLifetimeErrors => {
                        fix.emit::<FixableLifetimeErrors>(tcx, with_parent)
                    }
                    LintKind::NonPermissiveLifetimes => {
                        fix.emit::<NonPermissiveLifetimes>(tcx, with_parent)
                    }
                }
            });
    }
}
