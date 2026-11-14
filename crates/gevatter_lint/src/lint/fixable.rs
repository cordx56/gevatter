use super::SignatureLint;
use bridge::TyCtxt;
use bridge::hir::def_id::LocalDefId;
use bridge::lint;
use bridge::middle::ty::TypeVisitableExt;

lint::declare_tool_lint! {
    pub gevatter::FIXABLE_LIFETIME_ERRORS,
    Deny,
    "there are fixable lifetime errors for the function.",
    report_in_external_macro: false
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FixableLifetimeErrors;

impl FixableLifetimeErrors {
    pub(super) fn has_errors(tcx: TyCtxt<'_>, def_id: LocalDefId) -> bool {
        tcx.fn_sig(def_id).skip_binder().references_error() || tcx.mir_borrowck(def_id).is_err()
    }
}

impl SignatureLint for FixableLifetimeErrors {
    const LINT: &'static lint::Lint = FIXABLE_LIFETIME_ERRORS;
    const HELP: &'static str = "change the lifetime annotations as follows";

    fn message(function: &str) -> String {
        format!("lifetime annotations of `{function}` have to be changed to fix lifetime errors")
    }
}
