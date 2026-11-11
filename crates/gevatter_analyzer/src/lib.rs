#![feature(rustc_private)]

extern crate rustc_borrowck;
extern crate rustc_hir;
extern crate rustc_middle;

use rustc_borrowck::consumers as borrowck;
use rustc_hir::{
    def::DefKind,
    def_id::{CRATE_DEF_ID, LOCAL_CRATE, LocalDefId},
};
use rustc_middle::ty::{self, TyCtxt};

use indexmap::IndexMap;
use models::SkipReason;

pub mod callee;
pub mod outlives;
pub mod region_name;

pub use outlives::OutliveRelationReport;

pub fn report_outlives(tcx: TyCtxt<'_>) -> IndexMap<LocalDefId, OutliveRelationReport> {
    // proceeding to borrowck may occur ICE if there are well-formedness type errors
    for owner in tcx.hir_crate_items(()).owners() {
        if owner.def_id != CRATE_DEF_ID {
            tcx.ensure_ok().check_well_formed(owner.def_id);
        }
    }

    // check errors before borrowck
    let borrowck_skipped = if tcx.dcx().err_count() == 0 {
        None
    } else {
        Some(SkipReason::WellFormednessErrors)
    };

    let mut functions = IndexMap::new();
    for def_id in tcx.hir_body_owners() {
        // ignore non-free functions/methods
        if !matches!(tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn) {
            continue;
        }

        if let Some(reason) = borrowck_skipped {
            println!("TODO!");
            continue;
        }

        let errs_before_borrowck = tcx.dcx().err_count();
        let facts = borrowck::get_bodies_with_borrowck_facts(
            tcx,
            def_id,
            borrowck::ConsumerOptions::PoloniusOutputFacts,
        );
        let had_diagnostics = tcx.dcx().err_count() > errs_before_borrowck;
        let self_facts = facts.get(&def_id).expect("facts must have self def_id");

        // constraints extraction
        let input_facts = self_facts
            .input_facts
            .as_ref()
            .expect("Polonius borrowck must returns input facts here");

        let universal_regions: Vec<models::Region> = input_facts
            .universal_region
            .iter()
            .map(|vid| (*vid).into())
            .collect();

        // computing required outlives
        let known_outlives: outlives::OutliveRelations = input_facts
            .known_placeholder_subset
            .iter()
            .map(|(vid1, vid2)| ((*vid1).into(), (*vid2).into()))
            .collect();
        let base_subset_constraints: outlives::OutliveRelations = input_facts
            .subset_base
            .iter()
            .map(|(vid1, vid2, _)| ((*vid1).into(), (*vid2).into()))
            .collect();
        let required = outlives::compute_outlive_relations(
            &universal_regions,
            &known_outlives,
            &base_subset_constraints,
        );

        let names = region_name::Regions::construct(tcx, def_id, universal_regions.iter().copied());
        let named_required: Vec<_> = required
            .iter()
            .filter_map(
                |(sub, sup)| match (names.annotation(sub), names.annotation(sup)) {
                    (Some(sub), Some(sup)) => Some(models::LifetimeRelation {
                        shorter: sup.clone(),
                        longer: sub.clone(),
                    }),
                    _ => None,
                },
            )
            .collect();

        let def_path = tcx.def_path_str(def_id);
        let krate = tcx.crate_name(LOCAL_CRATE).to_string();
        let report = OutliveRelationReport {
            def_path,
            krate,
            had_diagnostics,
            required,
            named_required,
        };
        functions.insert(def_id, report);
    }
    functions.sort_by(|_, a, _, b| a.def_path.cmp(&b.def_path));
    functions
}
