//! Red-to-green contract for Strands of Undeath's entry and granted ability.

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, Target, TargetRequirement, TriggerCondition, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
    RAV_FULL_FIDELITY_DEFINITION_IDS,
};

fn rav_game() -> Game {
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(rav_attachment_triggered_ability_bindings());
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .expect("RAV game builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn strands_of_undeath_is_manifested_with_entry_discard_and_attached_regeneration() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STRANDS-OF-UNDEATH")
        .expect("Strands of Undeath definition exists");

    assert_eq!(definition.name, "Strands of Undeath");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Black])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "aura-attach-and-static-pt",
            "etb-target-player-discard",
            "attached-creature-granted-regeneration-activation",
        ]
    );

    let entry = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("entry discard trigger binding exists");
    assert_eq!(entry.ability.condition, TriggerCondition::EntersBattlefield);
    assert_eq!(entry.ability.targets, [TargetRequirement::Player]);
    assert_eq!(
        entry.ability.effects,
        [Effect::DiscardTargetPlayer { count: 1 }]
    );

    let attachment = rav_attachment_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("Aura attachment binding exists");
    let granted = attachment
        .granted_activated_abilities
        .into_iter()
        .find(|ability| ability.id == "attached-creature-regeneration")
        .expect("attached regeneration ability exists");
    assert_eq!(granted.mana_cost, ManaCost::with_colors(0, [Color::Black]));
    assert!(!granted.tap_cost);
    assert_eq!(granted.effects, [Effect::RegenerateSource]);
}

#[test]
fn strands_of_undeath_discards_on_entry_then_grants_regeneration_to_its_exact_creature() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = rav_game();
    let creature = game
        .put_on_battlefield(controller, "RAV-GOLIATH-SPIDER")
        .expect("creature setup");
    let strands = game
        .add_card(controller, "RAV-STRANDS-OF-UNDEATH", Zone::Hand)
        .expect("Aura setup");
    let discarded = game
        .add_card(opponent, "RAV-WATCHWOLF", Zone::Hand)
        .expect("opponent hand setup");
    game.grant_mana(controller, Color::Black, 5)
        .expect("Aura and activation mana");
    game.begin_game().expect("game begins");

    game.cast_spell(
        controller,
        CastRequest {
            card: strands,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts");
    pass_pair(&mut game);
    let choice = game
        .view_for_player(controller)
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("entry trigger requests a player target");
    assert_eq!(choice.ability, "etb-target-player-discard");
    game.submit_policy_move(
        controller,
        "test.strands-entry-discard-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: strands,
            ability: "etb-target-player-discard",
            targets: vec![Target::Player(opponent)],
        },
    )
    .expect("controller chooses the opponent");
    pass_pair(&mut game);

    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    game.activate_ability(
        controller,
        AbilityActivation {
            source: creature,
            ability_id: "attached-creature-regeneration",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("the live Aura grants its exact creature regeneration");
    pass_pair(&mut game);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDiscarded { player, card } if *player == opponent && *card == discarded
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == creature && *target == creature
    )));
    println!("strands_of_undeath_trace={:#?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("entry discard and attached regeneration preserve invariants");
}
