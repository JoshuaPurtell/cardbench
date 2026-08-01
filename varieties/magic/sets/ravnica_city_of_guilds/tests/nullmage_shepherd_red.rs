//! Regression for Nullmage Shepherd's four-creature artifact/enchantment removal.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, ManaCost, PlayerId, PolicyAction, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn nullmage_shepherd_has_its_exact_four_creature_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NULLMAGE-SHEPHERD")
        .expect("Nullmage Shepherd definition exists");
    assert_eq!(definition.name, "Nullmage Shepherd");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Green])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(4)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-four-untapped-controlled-creatures")
    );
    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-NULLMAGE-SHEPHERD"
                && binding.ability.id == "destroy-artifact-or-enchantment"
        })
        .expect("Nullmage Shepherd activation exists")
        .ability;
    assert_eq!(ability.mana_cost, ManaCost::new(0));
    assert!(!ability.tap_cost);
    assert_eq!(ability.additional_tap_creatures, 4);
    assert_eq!(
        ability.targets,
        vec![TargetRequirement::ArtifactOrEnchantment]
    );
}

#[test]
fn nullmage_shepherd_taps_four_distinct_creatures_before_destroying_target() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let shepherd = game
        .put_on_battlefield(PlayerId(0), "RAV-NULLMAGE-SHEPHERD")
        .expect("Shepherd setup");
    let cost_creatures = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
                .expect("cost creature setup")
        })
        .collect::<Vec<_>>();
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-GLASS-GOLEM")
        .expect("artifact target setup");
    game.begin_game().expect("game starts");

    game.submit_policy_move(
        PlayerId(0),
        "nullmage-shepherd-regression",
        PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: shepherd,
                ability_id: "destroy-artifact-or-enchantment",
                sacrifice_sources: vec![],
                additional_tap_creatures: cost_creatures.clone(),
                discard_cards: vec![],
                targets: vec![Target::Permanent(target)],
            },
        },
    )
    .expect("activation stacks");
    assert!(
        cost_creatures
            .iter()
            .all(|card| { game.object(*card).is_ok_and(|object| object.tapped) })
    );
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    println!("Nullmage Shepherd trace: {:?}", game.canonical_event_log());
    assert_eq!(
        game.canonical_event_log(),
        vec![
            "StepBegan { turn: 1, active_player: PlayerId(0), step: Untap }",
            "StepBegan { turn: 1, active_player: PlayerId(0), step: Upkeep }",
            "AdditionalCreatureTappedAsAbilityCost { player: PlayerId(0), source: ObjectId(1), permanent: ObjectId(2) }",
            "AdditionalCreatureTappedAsAbilityCost { player: PlayerId(0), source: ObjectId(1), permanent: ObjectId(3) }",
            "AdditionalCreatureTappedAsAbilityCost { player: PlayerId(0), source: ObjectId(1), permanent: ObjectId(4) }",
            "AdditionalCreatureTappedAsAbilityCost { player: PlayerId(0), source: ObjectId(1), permanent: ObjectId(5) }",
            "AbilityActivated { player: PlayerId(0), source: ObjectId(1), ability: \"destroy-artifact-or-enchantment\" }",
            "PolicyMoveSubmitted { player: PlayerId(0), policy: \"nullmage-shepherd-regression\", kind: ActivateAbility }",
            "PriorityPassed { player: PlayerId(0) }",
            "PriorityPassed { player: PlayerId(1) }",
            "CardDestroyed { source: ObjectId(1), card: ObjectId(6) }",
            "CardMoved { card: ObjectId(6), to: Graveyard }",
            "AbilityResolved { source: ObjectId(1), ability: \"destroy-artifact-or-enchantment\" }",
        ]
    );
    game.validate_invariants()
        .expect("four-creature activation trace stays valid");
}
