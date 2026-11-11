use bridge::TyCtxt;
use bridge::hir::{def::DefKind, def_id::LocalDefId};
use bridge::middle::ty;
use indexmap::IndexMap;
use models::{Binding, SignatureBinding};
use ty::{TypeVisitable, TypeVisitableExt, TypeVisitor};

pub fn signature_bindings(tcx: TyCtxt<'_>) -> IndexMap<LocalDefId, SignatureBinding> {
    tcx.hir_crate_items(())
        .owners()
        .map(|owner| owner.def_id)
        .filter(|def_id| matches!(tcx.def_kind(*def_id), DefKind::Fn | DefKind::AssocFn))
        .filter_map(|def_id| signature_binding(tcx, def_id).map(|binding| (def_id, binding)))
        .collect()
}

fn signature_binding(tcx: TyCtxt<'_>, def_id: LocalDefId) -> Option<SignatureBinding> {
    let signature =
        tcx.liberate_late_bound_regions(def_id.to_def_id(), tcx.fn_sig(def_id).skip_binder());
    if signature.references_error() {
        return None;
    }
    let mut slots = SlotCollector(Vec::new());
    signature.inputs_and_output.visit_with(&mut slots);
    Some(slots.0.into_iter().collect())
}

struct SlotCollector(Vec<Option<Binding>>);
impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for SlotCollector {
    fn visit_region(&mut self, region: ty::Region<'tcx>) {
        self.0.push(match region.kind() {
            ty::ReEarlyParam(_) => Some(Binding::Early),
            ty::ReLateParam(_) => Some(Binding::Late),
            _ => None,
        });
    }
}
