use cardbench_magic_engine::{
    AbilityActivation, ContinuousChange, DecisionSelection, Duration, Game, GameEvent, Keyword,
    ObjectId, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture builds")
}

fn begin_and_advance_to_first_main_with_fixture_haste(game: &mut Game, broker: ObjectId) {
    game.begin_game().expect("fixture begins");
    game.add_continuous_effect(
        broker,
        broker,
        ContinuousChange::AddKeyword(Keyword::Haste),
        Duration::EndOfTurn(game.turn),
    )
    .expect("fixture haste");
    while game.step != Step::PrecombatMain {
        let player = game.priority;
        game.pass_priority(player).expect("advance to first main");
    }
}

#[test]
fn lore_broker_draws_every_player_before_collecting_private_discards_as_one_batch() {
    let mut game = game();
    let broker = game
        .put_on_battlefield(PlayerId(0), "RAV-LORE-BROKER")
        .expect("Lore Broker setup");
    let first_discard = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("first hand");
    let second_discard = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("second hand");
    let first_draw = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("first library");
    let second_draw = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("second library");
    begin_and_advance_to_first_main_with_fixture_haste(&mut game, broker);

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: broker,
            ability_id: "each-player-draws-then-discards",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Lore Broker activation");
    game.pass_priority(PlayerId(0)).expect("first pass");
    game.pass_priority(PlayerId(1))
        .expect("second pass opens choices");

    assert_eq!(game.zone_of(first_draw), Some(Zone::Hand));
    assert_eq!(game.zone_of(second_draw), Some(Zone::Hand));
    let first_choice = game
        .view_for_player(PlayerId(0))
        .expect("first view")
        .pending_decision
        .expect("first private choice");
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("second view")
            .pending_decision
            .is_none()
    );
    game.submit_decision(
        PlayerId(0),
        first_choice.id,
        DecisionSelection::Objects(vec![first_discard]),
    )
    .expect("first private discard choice");
    assert_eq!(game.zone_of(first_discard), Some(Zone::Hand));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::CardDiscarded { .. }))
    );

    let second_choice = game
        .view_for_player(PlayerId(1))
        .expect("second view")
        .pending_decision
        .expect("second private choice");
    game.submit_decision(
        PlayerId(1),
        second_choice.id,
        DecisionSelection::Objects(vec![second_discard]),
    )
    .expect("second private discard choice commits the batch");
    assert_eq!(game.zone_of(first_discard), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(second_discard), Some(Zone::Graveyard));
    let trace = game.canonical_event_log();
    eprintln!("Lore Broker trace={trace:?}");
    let first_discard_index = trace
        .iter()
        .position(|event| event.contains("CardDiscarded"))
        .expect("discard receipt");
    let resolved_index = trace
        .iter()
        .position(|event| event.contains("AbilityResolved"))
        .expect("terminal ability receipt");
    assert!(first_discard_index < resolved_index);
    game.validate_invariants()
        .expect("Lore Broker invariant state");
}
