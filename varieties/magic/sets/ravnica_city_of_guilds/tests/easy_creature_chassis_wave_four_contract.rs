//! Public contract for the fourth bounded RAV creature-chassis batch.
//!
//! These compatibility definitions deliberately expose normal casting and base
//! characteristics only. Goblin Fire Fiend, Thoughtpicker Witch, and
//! Vindictive Mob are separately audited by their full-fidelity contracts and
//! are excluded from the bounded matrix below. Woebringer Demon is separately
//! promoted by its explicit each-upkeep sacrifice contract.

use std::collections::BTreeSet;

use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Explicit base-fact matrix is intentionally audit-friendly.
fn fourth_creature_chassis_batch_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();

    let excruciator = definitions
        .iter()
        .find(|definition| definition.id == "RAV-EXCRUCIATOR")
        .expect("Excruciator definition exists");
    assert_eq!(
        excruciator.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "damage-cannot-be-prevented"
        ]
    );

    let brute = definitions
        .iter()
        .find(|definition| definition.id == "RAV-SELL-SWORD-BRUTE")
        .expect("Sell-Sword Brute definition exists");
    assert_eq!(
        brute.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "dies-deal-two-to-controller"
        ]
    );
}

#[test]
fn public_fourth_chassis_scenarios_cover_costs_stack_zones_and_base_pt() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_demon_witch_creature_chassis",
        "rav_shade_mob_creature_chassis",
        "rav_riftcutter_excruciator_creature_chassis",
        "rav_fire_fiend_brute_creature_chassis",
        "rav_goblin_fire_fiend_haste_compatibility",
        "rav_woebringer_demon_flying_compatibility",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
