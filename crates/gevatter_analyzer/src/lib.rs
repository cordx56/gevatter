#![feature(rustc_private)]

use bridge::borrowck;
use bridge::hir::{
    def::DefKind,
    def_id::{LOCAL_CRATE, LocalDefId},
};
use bridge::{FromRustc, TyCtxt};
use indexmap::IndexMap;

pub mod binding;
pub mod outlives;
pub mod region_name;

pub use outlives::OutliveRelationReport;

pub fn report_outlives(tcx: TyCtxt<'_>) -> IndexMap<LocalDefId, OutliveRelationReport> {
    // proceeding to borrowck may occur ICE if there are well-formedness type errors
    // check errors before borrowck
    let borrowck_skipped = bridge::check_well_formedness(tcx);

    let krate = tcx.crate_name(LOCAL_CRATE).to_string();
    let mut functions = IndexMap::new();
    for def_id in tcx.hir_body_owners() {
        // ignore non-free functions/methods
        if !matches!(tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn) {
            continue;
        }
        let def_path = tcx.def_path_str(def_id);

        if borrowck_skipped.is_some() {
            functions.insert(
                def_id,
                OutliveRelationReport {
                    def_path,
                    krate: krate.clone(),
                    had_diagnostics: true,
                    borrowck_skipped,
                    required: Vec::new(),
                },
            );
            continue;
        }

        let errs_before_borrowck = tcx.dcx().err_count();
        let facts = borrowck::get_bodies_with_borrowck_facts(
            tcx,
            def_id,
            borrowck::ConsumerOptions::PoloniusInputFacts,
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
            .map(|vid| FromRustc::from_rustc(*vid))
            .collect();

        // computing required outlives
        let known_outlives: outlives::OutliveRelations = input_facts
            .known_placeholder_subset
            .iter()
            .map(|(vid1, vid2)| (FromRustc::from_rustc(*vid1), FromRustc::from_rustc(*vid2)))
            .collect();
        let base_subset_constraints: outlives::OutliveRelations = input_facts
            .subset_base
            .iter()
            .map(|(vid1, vid2, _)| (FromRustc::from_rustc(*vid1), FromRustc::from_rustc(*vid2)))
            .collect();
        let names = region_name::Regions::construct(tcx, def_id, universal_regions.iter().copied());
        let required = outlives::compute_outlive_relations(
            &universal_regions,
            &known_outlives,
            &base_subset_constraints,
        )
        .into_iter()
        .map(|(longer, shorter)| models::RequiredOutlive {
            longer: names.resolve(longer),
            shorter: names.resolve(shorter),
        })
        .collect();

        let report = OutliveRelationReport {
            def_path,
            krate: krate.clone(),
            had_diagnostics,
            borrowck_skipped: None,
            required,
        };
        functions.insert(def_id, report);
    }
    functions.sort_by(|_, a, _, b| a.def_path.cmp(&b.def_path));
    functions
}
