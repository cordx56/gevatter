use bridge::TyCtxt;
use bridge::hir::{def::DefKind, def_id::LocalDefId};
use bridge::middle::ty;
use indexmap::IndexMap;
use models::{Binding, Owner, SignatureBinding};
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
    let mut collector = SlotCollector {
        parent_count: tcx.generics_of(def_id).parent_count,
        slots: Vec::new(),
    };
    signature.inputs_and_output.visit_with(&mut collector);
    Some(collector.slots.into_iter().collect())
}

struct SlotCollector {
    parent_count: usize,
    slots: Vec<Option<Binding>>,
}
impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for SlotCollector {
    fn visit_region(&mut self, region: ty::Region<'tcx>) {
        self.slots.push(match region.kind() {
            ty::ReEarlyParam(param) if (param.index as usize) < self.parent_count => {
                Some(Binding::Early(Owner::Parent))
            }
            ty::ReEarlyParam(_) => Some(Binding::Early(Owner::Function)),
            ty::ReLateParam(_) => Some(Binding::Late),
            _ => None,
        });
    }
}
