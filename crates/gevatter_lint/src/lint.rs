use bridge::errors::{Applicability, DiagDecorator};
use bridge::hir::{def::DefKind, def_id::LocalDefId};
use bridge::lint::{Lint, LintStore};
use bridge::span::{BytePos, SourceFile, Span};
use bridge::{FromRustc, TyCtxt};
use models::{FunctionKey, Generality, HeaderEdit, HeaderEdits, HeaderKind};

mod fixable;
mod permissive;

pub use fixable::*;
pub use permissive::*;

/// Gevatter has to be registered as crate tool
pub fn crate_attributes() -> [String; 2] {
    ["feature(register_tool)", "register_tool(gevatter)"].map(String::from)
}

pub fn register_lints(store: &mut LintStore) {
    store.register_lints(&[FIXABLE_LIFETIME_ERRORS, NON_PERMISSIVE_LIFETIMES]);
}

trait SignatureLint {
    const LINT: &'static Lint;
    const HELP: &'static str;

    fn message(function: &str) -> String;
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

struct SignatureFix {
    def_id: LocalDefId,
    generality: Option<Generality>,
    parts: Vec<(Span, String)>,
}
impl SignatureFix {
    fn new(tcx: TyCtxt<'_>, def_id: LocalDefId, edits: &HeaderEdits) -> Option<Self> {
        let edit = edits
            .get(&FunctionKey::from_rustc((tcx, def_id)))
            .filter(|edit| edit.kind == HeaderKind::Function)?;
        let parent = edit.parent.as_ref().map(|parent| edits.get(parent));
        let related: Vec<_> = std::iter::once(Some(edit))
            .chain(parent)
            .collect::<Option<_>>()?;
        let source = tcx
            .sess
            .source_map()
            .lookup_source_file(tcx.def_span(def_id).lo());
        if !related.iter().all(|edit| is_current(tcx, &source, edit)) {
            tracing::warn!(
                "the source of {} differs from the edited one",
                edit.def_path
            );
            return None;
        }
        let parts = related
            .iter()
            .flat_map(|edit| &edit.replacements)
            .map(|replacement| {
                (
                    span_in(&source, replacement.span),
                    replacement.replace_with.clone(),
                )
            })
            .collect();
        Some(Self {
            def_id,
            generality: edit.generality,
            parts,
        })
    }

    fn emit<L: SignatureLint>(self, tcx: TyCtxt<'_>) {
        let Self { def_id, parts, .. } = self;
        let span = tcx
            .def_ident_span(def_id)
            .unwrap_or_else(|| tcx.def_span(def_id));
        let message = L::message(&tcx.def_path_str(def_id));
        tcx.emit_node_span_lint(
            L::LINT,
            tcx.local_def_id_to_hir_id(def_id),
            span,
            DiagDecorator(|diag| {
                diag.primary_message(message);
                diag.multipart_suggestion(L::HELP, parts, Applicability::MachineApplicable);
            }),
        );
    }
}

pub fn emit_lints(tcx: TyCtxt<'_>, edits: &HeaderEdits) {
    tcx.hir_crate_items(())
        .owners()
        .map(|owner| owner.def_id)
        .filter(|def_id| matches!(tcx.def_kind(*def_id), DefKind::Fn | DefKind::AssocFn))
        .filter_map(|def_id| SignatureFix::new(tcx, def_id, edits))
        .for_each(|fix| match fix.generality {
            _ if FixableLifetimeErrors::has_errors(tcx, fix.def_id) => {
                fix.emit::<FixableLifetimeErrors>(tcx)
            }
            Some(Generality::Equivalent) => {}
            Some(Generality::LessGeneral | Generality::Incomparable) => {
                fix.emit::<FixableLifetimeErrors>(tcx)
            }
            Some(Generality::MoreGeneral) | None => fix.emit::<NonPermissiveLifetimes>(tcx),
        });
}
