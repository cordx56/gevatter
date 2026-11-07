use crate::{TyCtxt, hir::def_id::LocalDefId, ty};
use models::{Region, RegionKind, RegionName};
use std::collections::BTreeMap;

/// A map from [`Region`] to (its name, its kind)
pub type Regions = BTreeMap<Region, (Option<RegionName>, RegionKind)>;

pub fn collect_region_names(
    tcx: TyCtxt<'_>,
    def_id: LocalDefId,
    universal: impl Iterator<Item = Region>,
) -> Regions {
    let early_bound = ty::GenericArgs::identity_for_item(tcx, def_id)
        .regions()
        .map(|region| match region.kind() {
            ty::ReEarlyParam(param) => {
                (param.name.as_str() != "'_").then(|| RegionName::from(param.name.as_str()))
            }
            _ => None,
        });
    let sig = tcx.fn_sig(def_id).skip_binder();
    let late_bound = sig.bound_vars().into_iter().map(|var| match var {
        ty::BoundVariableKind::Region(kind) => match kind {
            ty::BoundRegionKind::Named(id) => {
                let name = tcx.item_name(id);
                (name.as_str() != "'_").then(|| RegionName::from(name.as_str()))
            }
            _ => None,
        },
        _ => None,
    });

    // FIXME: this chaining order is depends on rustc's internal implementation. FRAGILE!
    let ordered: Vec<_> = std::iter::once((Some(RegionName::from("'static")), RegionKind::Static))
        .chain(early_bound.map(|name| (name, RegionKind::EarlyBound)))
        .chain(late_bound.map(|name| (name, RegionKind::LateBound)))
        .chain(std::iter::once((None, RegionKind::Body)))
        .collect();

    universal
        .map(|region| {
            let ord = ordered
                .get(region.as_u32() as usize)
                .map_or((None, RegionKind::Anonymous), |(name, kind)| {
                    (name.clone(), *kind)
                });
            (region, (ord.clone()))
        })
        .collect()
}
