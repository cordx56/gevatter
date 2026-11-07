use crate::{CRATE_DEF_ID, TyCtxt, borrowck, hir::def::DefKind};
use models::SkipReason;

pub mod outlives;
pub mod region_name;

/*
pub fn collect_reports(tcx: TyCtxt<'_>) {
    // proceeding to borrowck may occur ICE if there are wf type errors
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
        let self_facts = facts
            .get(&def_id)
            .expect("facts must have self def_id ({def_id:?})");

        // constraints extraction
        let input_facts = self_facts
            .input_facts
            .as_ref()
            .expect("Polonius borrowck must returns input facts here");
        let universal_regions: Vec<_> = input_facts
            .universal_region
            .iter()
            .map(|vid| (*vid).into())
            .collect();
        let known_outlives: Vec<_> = input_facts
            .known_placeholder_subset
            .iter()
            .map(|(vid1, vid2)| ((*vid1).into(), (*vid2).into()))
            .collect();
        let base_subset_constraints: Vec<_> = input_facts
            .subset_base
            .iter()
            .map(|(vid1, vid2, _)| ((*vid1).into(), (*vid2).into()))
            .collect();

        let required_outlives = outlives::compute_outlive_relations(
            &universal_regions,
            &known_outlives,
            &base_subset_constraints,
        );

        let names = region_name::collect_region_names(
            tcx,
            def_id,
            input_facts.universal_region.iter().map(|v| (*v).into()),
        );
        let named_required = required_outlives.iter().map(|(a, b)| {
            names.get(a).map(|)
        })
    }
}
*/
