//! Regression guard for the former third bounded RAV creature-chassis batch.
//!
//! Every historical member has since been promoted to a dedicated full-fidelity
//! contract, so this test prevents the retired base-only matrix from silently
//! reclassifying one of those cards as a compatibility chassis.

use std::collections::BTreeSet;

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, run_all_scenarios};

#[test]
fn third_creature_chassis_batch_has_no_remaining_bounded_members() {
    for id in [
        "RAV-PRIMORDIAL-SAGE",
        "RAV-TROPHY-HUNTER",
        "RAV-VINELASHER-KUDZU",
        "RAV-WOODWRAITH-CORRUPTER",
    ] {
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id),
            "{id} must remain covered by its full-fidelity contract"
        );
    }
}

#[test]
fn public_third_chassis_scenarios_cover_costs_stack_zones_and_base_pt() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_wave_three_green_creature_chassis",
        "rav_wave_three_guild_creature_chassis",
        "rav_wave_three_boros_golgari_creature_chassis",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
