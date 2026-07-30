//! Ability-complete contract for simple RAV cards.

use cardbench_magic_engine::{Effect, TargetRequirement};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn full_fidelity_manifest_records_only_ability_complete_cards() {
    assert_eq!(
        RAV_FULL_FIDELITY_DEFINITION_IDS,
        [
            "RAV-CHAR",
            "RAV-LIGHTNING-HELIX",
            "RAV-LAST-GASP",
            "RAV-ELVES-OF-DEEP-SHADOW",
            "RAV-BOROS-RECRUIT",
        ]
    );
    let definitions = card_definitions();
    let exact = [
        (
            "RAV-CHAR",
            vec![
                Effect::DealDamage {
                    amount: 4,
                    target: TargetRequirement::PlayerOrCreature,
                },
                Effect::DealDamageController { amount: 2 },
            ],
        ),
        (
            "RAV-LIGHTNING-HELIX",
            vec![
                Effect::DealDamage {
                    amount: 3,
                    target: TargetRequirement::PlayerOrCreature,
                },
                Effect::GainLifeController { amount: 3 },
            ],
        ),
        (
            "RAV-LAST-GASP",
            vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -3,
                toughness: -3,
            }],
        ),
    ];
    for (id, effects) in exact {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .expect("definition exists");
        assert_eq!(definition.supported_rules[0], "full-rules-fidelity", "{id}");
        assert_eq!(definition.effects, effects, "{id}");
    }
    let elves = definitions
        .iter()
        .find(|definition| definition.id == "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("Elves of Deep Shadow definition exists");
    assert_eq!(elves.supported_rules[0], "full-rules-fidelity");
    let recruit = definitions
        .iter()
        .find(|definition| definition.id == "RAV-BOROS-RECRUIT")
        .expect("Boros Recruit definition exists");
    assert_eq!(recruit.supported_rules[0], "full-rules-fidelity");
}

#[test]
fn full_fidelity_card_scenarios_emit_their_complete_effect_receipts() {
    let results = run_all_scenarios().expect("public RAV scenarios run");
    for (id, markers) in [
        (
            "rav_stack_lightning_helix",
            ["DamageDealtToPlayer", "LifeGained"],
        ),
        (
            "rav_last_gasp_state_based_action",
            ["ContinuousEffectCreated", "StateBasedAction"],
        ),
        (
            "rav_elves_of_deep_shadow_complete_mana_ability",
            ["ManaAdded", "DamageDealtToPlayer"],
        ),
        (
            "rav_boros_recruit_hybrid_first_strike",
            ["SpellCast", "FirstStrikeCombatDamage"],
        ),
    ] {
        let result = results
            .iter()
            .find(|result| result.id == id)
            .expect("scenario exists");
        for marker in markers {
            assert!(
                result.event_log.iter().any(|event| event.contains(marker)),
                "{id} lacks {marker}"
            );
        }
    }
}
