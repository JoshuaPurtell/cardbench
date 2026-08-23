//! Red-first public contract for complete RAV Elves of Deep Shadow support.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaAbilityOutput, ManaCost};
use cardbench_magic_rav::{card_definitions, rav_mana_ability_bindings, run_all_scenarios};

#[test]
fn elves_of_deep_shadow_has_its_public_card_and_black_mana_binding() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("complete Elves of Deep Shadow definition");
    assert_eq!(definition.name, "Elves of Deep Shadow");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::Green])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Green]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(definition.power, Some(1));
    assert_eq!(definition.toughness, Some(1));
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "bound-tap-black-mana-ability",
            "source-aware-controller-damage",
        ]
    );

    let binding = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("complete Elves of Deep Shadow mana binding");
    assert!(binding.ability.tap_cost);
    assert_eq!(
        binding.ability.output,
        ManaAbilityOutput::Fixed(Color::Black)
    );
    assert_eq!(binding.ability.amount, 1);
    assert_eq!(binding.ability.life_payment, None);
    assert_eq!(binding.ability.controller_damage, Some(1));
}

#[test]
fn public_elves_scenario_records_black_mana_source_damage_and_tapped_rejection() {
    let scenario = run_all_scenarios()
        .expect("public RAV scenarios execute")
        .into_iter()
        .find(|result| result.id == "rav_elves_of_deep_shadow_complete_mana_ability")
        .expect("complete Elves of Deep Shadow public scenario");

    assert_eq!(scenario.digest, "fnv1a64:fbc77aaaa3194b51");
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("BoundManaAbilityActivated")
            && event.contains("ability: \"produce-black-and-damage-controller\"")
            && event.contains("color: Black")
            && event.contains("amount: 1")
    }));
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("ManaAdded { player: PlayerId(0), color: Black, amount: 1 }")
    }));
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("DamageDealtToPlayer")
            && event.contains("source: ObjectId(1)")
            && event.contains("player: PlayerId(0)")
            && event.contains("amount: 1")
    }));
    assert!(
        !scenario
            .event_log
            .iter()
            .any(|event| event.contains("ManaAbilityLifePaid")),
        "the card's damage must not be rewritten as a life-payment receipt"
    );
}
