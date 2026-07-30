use std::collections::BTreeSet;

use cardbench_magic_engine::{Color, ManaAbilityOutput};
use cardbench_magic_rav::{card_definitions, rav_mana_ability_bindings, run_all_scenarios};

#[test]
fn birds_of_paradise_has_one_bounded_five_color_tap_mana_binding() {
    let definitions = card_definitions();
    let birds = definitions
        .iter()
        .find(|definition| definition.id == "RAV-BIRDS-OF-PARADISE")
        .expect("RAV Birds of Paradise compatibility definition");
    assert_eq!(birds.name, "Birds of Paradise");
    assert_eq!(birds.power, Some(0));
    assert_eq!(birds.toughness, Some(1));
    assert_eq!(
        birds.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "bound-tap-choice-mana-ability",
        ]
    );

    let bindings = rav_mana_ability_bindings();
    assert_eq!(bindings.len(), 1);
    let binding = &bindings[0];
    assert_eq!(binding.card_definition, birds.id);
    assert_eq!(binding.ability.id, "produce-one-color");
    assert!(binding.ability.tap_cost);
    assert_eq!(binding.ability.amount, 1);
    assert_eq!(binding.ability.life_payment, None);
    assert_eq!(
        binding.ability.output,
        ManaAbilityOutput::Choice(BTreeSet::from(Color::ALL))
    );
}

#[test]
fn public_birds_scenario_records_real_turn_progression_and_chosen_blue_mana() {
    let scenario = run_all_scenarios()
        .expect("public RAV scenarios execute")
        .into_iter()
        .find(|result| result.id == "rav_birds_of_paradise_bound_mana_ability")
        .expect("Birds of Paradise public scenario");

    assert_eq!(scenario.digest, "fnv1a64:7115881e15dd0d84");
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("StepBegan { turn: 2, active_player: PlayerId(1), step: Upkeep }")
    }));
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("BoundManaAbilityActivated")
            && event.contains("player: PlayerId(0)")
            && event.contains("ability: \"produce-one-color\"")
            && event.contains("color: Blue")
            && event.contains("amount: 1")
            && event.contains("tapped: true")
    }));
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("ManaAdded { player: PlayerId(0), color: Blue, amount: 1 }")
    }));
}
