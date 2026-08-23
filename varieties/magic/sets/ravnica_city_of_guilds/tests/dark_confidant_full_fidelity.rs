use cardbench_magic_engine::{Game, GameEvent, PlayerId, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn dark_confidant_reveals_moves_and_loses_exact_top_card_mana_value_at_upkeep() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger game builds");
    let confidant = game
        .put_on_battlefield(controller, "RAV-DARK-CONFIDANT")
        .expect("Confidant setup");
    game.set_entered_turn_for_setup(confidant, 0)
        .expect("Confidant predates upkeep");
    let revealed = game
        .add_card(controller, "RAV-WATCHWOLF", Zone::Library)
        .expect("top library setup");

    game.begin_game().expect("game begins at upkeep");
    assert_eq!(game.stack.len(), 1, "upkeep trigger stacks before priority");
    game.pass_priority(controller).expect("controller passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and trigger resolves");

    println!("Dark Confidant trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(revealed), Some(Zone::Hand));
    assert_eq!(game.player(controller).unwrap().life, 18);
    let revealed_event = game
        .event_log
        .iter()
        .position(
            |event| matches!(event, GameEvent::CardRevealed { card, .. } if *card == revealed),
        )
        .expect("public reveal receipt");
    let life_loss = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::LifeLost { source, player, amount } if *source == confidant && *player == controller && *amount == 2))
        .expect("mana-value life loss receipt");
    assert!(revealed_event < life_loss);
    game.validate_invariants()
        .expect("upkeep reveal trace remains valid");
}

#[test]
fn dark_confidant_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-DARK-CONFIDANT"));
}
