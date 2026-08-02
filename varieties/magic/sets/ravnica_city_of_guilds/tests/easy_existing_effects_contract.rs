//! Public RAV compatibility boundary for cards that reuse existing engine effects.
//!
//! These contracts assert the represented semantic slices, not whole-card
//! fidelity: omitted attachment, recursion, cost, and keyword behavior stays
//! out of `supported_rules` and cannot be mistaken for an engine feature.

use cardbench_magic_engine::{Color, Effect, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    run_all_scenarios,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("missing RAV definition {id}"))
}

#[test]
fn seeds_of_strength_preserves_three_separate_target_modifiers() {
    let seeds = definition("RAV-SEEDS-OF-STRENGTH");
    assert_eq!(
        seeds.mana_cost,
        ManaCost::with_colors(0, [Color::Green, Color::White])
    );
    assert_eq!(
        seeds.supported_rules,
        [
            "full-rules-fidelity",
            "three-targeted-layer-7-modifiers",
            "partial-target-resolution",
        ]
    );
    assert_eq!(
        seeds.effects,
        vec![
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
        ]
    );
}

#[test]
fn bounded_effect_slices_state_only_the_semantics_that_are_executable() {
    let caress = definition("RAV-DRYADS-CARESS");
    assert_eq!(caress.supported_rules, ["controller-life-gain"]);
    assert_eq!(
        caress.effects,
        vec![Effect::GainLifeController { amount: 1 }]
    );

    let conclusion = definition("RAV-FIERY-CONCLUSION");
    assert_eq!(
        conclusion.supported_rules,
        [
            "full-rules-fidelity",
            "additional-sacrifice-controlled-creature-cost",
            "targeted-creature-damage",
        ]
    );
    assert_eq!(
        conclusion.effects,
        vec![Effect::DealDamage {
            amount: 5,
            target: TargetRequirement::Creature,
        }]
    );
}

#[test]
fn gaze_of_gorgon_uses_the_typed_regeneration_and_combat_history_substrate() {
    assert_eq!(
        executable_definition_id_for_collector(246),
        Ok("RAV-GAZE-OF-THE-GORGON")
    );
    let gaze = definition("RAV-GAZE-OF-THE-GORGON");
    assert_eq!(
        gaze.effects,
        [Effect::RegenerateTargetCreatureAndScheduleCombatHistoryDestruction]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-GAZE-OF-THE-GORGON"),
        "the exact typed implementation belongs in the positive manifest"
    );
}

#[test]
fn public_scenarios_exercise_each_existing_effect_card_slice() {
    let scenario_ids = run_all_scenarios()
        .expect("RAV scenarios run")
        .into_iter()
        .map(|result| result.id)
        .collect::<std::collections::BTreeSet<_>>();
    for id in [
        "rav_seeds_of_strength_three_modifiers",
        "rav_fists_of_ironwood_tokens",
        "rav_dryads_caress_life",
        "rav_fiery_conclusion_sacrifice_damage",
    ] {
        assert!(scenario_ids.contains(id), "missing public scenario {id}");
    }
    assert!(
        !scenario_ids.contains("rav_gaze_of_the_gorgon_modifier"),
        "the removed false semantic slice must not have a replay fixture"
    );
}
