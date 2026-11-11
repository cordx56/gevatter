use crate::{LocalDefId, TyCtxt, ty};
use models::{LifetimeAnnotation, Region, RegionKind, ResolvedRegion};
use std::collections::BTreeMap;

/// A map from [`Region`] to its name and its kind.
/// Used to get name and kind which is related to the region.
pub struct Regions(BTreeMap<Region, (Option<LifetimeAnnotation>, RegionKind)>);
impl Regions {
    pub fn annotation(&self, region: &Region) -> Option<&LifetimeAnnotation> {
        self.0.get(region).map(|(name, _)| name.as_ref()).flatten()
    }

    pub fn resolve(&self, region: Region) -> ResolvedRegion {
        let (name, kind) = self
            .0
            .get(&region)
            .cloned()
            .unwrap_or((None, RegionKind::Anonymous));
        ResolvedRegion { region, name, kind }
    }

    pub fn construct(
        tcx: TyCtxt<'_>,
        def_id: LocalDefId,
        universal: impl Iterator<Item = Region>,
    ) -> Regions {
        let early_bound = ty::GenericArgs::identity_for_item(tcx, def_id)
            .regions()
            .map(|region| match region.kind() {
                ty::ReEarlyParam(param) => param
                    .is_named()
                    .then(|| LifetimeAnnotation::from(param.name.as_str())),
                _ => None,
            });
        let sig = tcx.fn_sig(def_id).skip_binder();
        let late_bound = sig.bound_vars().into_iter().map(|var| match var {
            ty::BoundVariableKind::Region(kind) => match kind {
                ty::BoundRegionKind::Named(id) => {
                    let name = tcx.item_name(id);
                    // FIXME: is this needed?
                    (name.as_str() != "'_").then(|| LifetimeAnnotation::from(name.as_str()))
                }
                _ => None,
            },
            _ => None,
        });

        // FIXME: this chaining order is depends on rustc's internal implementation. FRAGILE!
        let ordered: Vec<_> = std::iter::once((
            Some(LifetimeAnnotation::from("'static")),
            RegionKind::Static,
        ))
        .chain(early_bound.map(|name| (name, RegionKind::EarlyBound)))
        .chain(late_bound.map(|name| (name, RegionKind::LateBound)))
        .chain(std::iter::once((None, RegionKind::Body)))
        .collect();

        let regions = universal
            .map(|region| {
                let ord = ordered
                    .get(region.as_u32() as usize)
                    .map_or((None, RegionKind::Anonymous), |(name, kind)| {
                        (name.clone(), *kind)
                    });
                (region, (ord.clone()))
            })
            .collect();
        Regions(regions)
    }
}
