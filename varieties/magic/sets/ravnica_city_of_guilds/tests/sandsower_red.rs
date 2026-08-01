//! Red regression for Sandsower's three-creature tap activation.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, ManaCost, PlayerId, PolicyAction, Target,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn sandsower_has_its_exact_three_creature_tap_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SANDSOWER")
        .expect("Sandsower definition exists");
    assert_eq!(definition.name, "Sandsower");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::White])
    );
    assert_eq!(definition.colors, [Color::White].into_iter().collect());
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-three-untapped-controlled-creatures")
    );
    assert!(definition.supported_rules.contains(&"tap-target-creature"));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-SANDSOWER" && binding.ability.id == "tap-target-creature"
    }));
}

#[test]
fn sandsower_taps_three_distinct_creatures_then_taps_its_target_on_resolution() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let sandsower = game
        .put_on_battlefield(PlayerId(0), "RAV-SANDSOWER")
        .expect("Sandsower enters");
    let first = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("first cost creature enters");
    let second = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("second cost creature enters");
    let third = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("third cost creature enters");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature enters");
    for permanent in [sandsower, first, second, third, target] {
        game.set_entered_turn_for_setup(permanent, 0)
            .expect("fixture creature predates measured turn");
    }
    game.begin_game().expect("game starts");
    game.submit_policy_move(
        PlayerId(0),
        "sandsower-regression",
        PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: sandsower,
                ability_id: "tap-target-creature",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![first, second, third],
                discard_cards: vec![],
                targets: vec![Target::Permanent(target)],
            },
        },
    )
    .expect("Sandsower activation stacks");
    for cost_creature in [first, second, third] {
        assert!(
            game.object(cost_creature)
                .expect("cost creature remains")
                .tapped
        );
    }
    assert!(!game.object(target).expect("target remains").tapped);
    game.pass_priority(PlayerId(0))
        .expect("controller passes Sandsower ability");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes Sandsower ability");
    println!("Sandsower trace: {:?}", game.canonical_event_log());
    assert!(game.object(target).expect("target remains").tapped);
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::AdditionalCreatureTappedAsAbilityCost { source, .. }
                    if *source == sandsower
            ))
            .count(),
        3
    );
    game.validate_invariants()
        .expect("Sandsower trace preserves the cost and stack invariants");
}
