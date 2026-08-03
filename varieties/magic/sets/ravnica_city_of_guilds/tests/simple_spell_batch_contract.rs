//! Public RAV contracts for the low-complexity spell batch.
//!
//! Four definitions intentionally expose only Transmute, while Rain of Embers
//! exposes the complete target-free global-damage operation. This keeps the
//! catalog executable boundary explicit instead of making an unsupported spell
//! resolve as a no-op.

use cardbench_magic_engine::{Color, Effect, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("missing RAV definition {id}"))
}

#[test]
fn rain_of_embers_is_exactly_the_target_free_global_damage_slice() {
    let rain = definition("RAV-RAIN-OF-EMBERS");
    assert_eq!(rain.mana_cost, ManaCost::with_colors(1, [Color::Red]));
    assert_eq!(
        rain.supported_rules,
        ["full-rules-fidelity", "global-creature-and-player-damage"]
    );
    assert_eq!(
        rain.effects,
        vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 1 }]
    );
    assert!(
        rain.effects
            .iter()
            .all(|effect| effect.target_requirement().is_none())
    );
}

#[test]
fn public_scenarios_exercise_every_new_compatibility_definition() {
    let scenario_ids = run_all_scenarios()
        .expect("RAV scenarios run")
        .into_iter()
        .map(|result| result.id)
        .collect::<std::collections::BTreeSet<_>>();
    for id in [
        "rav_rain_of_embers_global_damage",
        "rav_dimir_machinations_transmute",
        "rav_shred_memory_transmute",
        "rav_clutch_of_the_undercity_transmute",
        "rav_clutch_of_the_undercity_permanent_bounce",
        "rav_blood_funnel_cast_trigger",
        "rav_perplex_transmute",
    ] {
        assert!(scenario_ids.contains(id), "missing public scenario {id}");
    }
}
