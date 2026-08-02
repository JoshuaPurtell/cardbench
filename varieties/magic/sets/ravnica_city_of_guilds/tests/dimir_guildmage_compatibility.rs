use cardbench_magic_engine::{
    AbilityActivation, Color, Game, GameEvent, PlayerId, RulesError, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn rav_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn activation(
    source: cardbench_magic_engine::ObjectId,
    ability_id: &'static str,
) -> AbilityActivation {
    AbilityActivation {
        source,
        ability_id,
        sacrifice_sources: vec![],
        additional_tap_creatures: vec![],
        discard_cards: vec![],
        targets: vec![Target::Player(PlayerId(1))],
    }
}

#[test]
fn dimir_guildmage_resolves_target_player_draw_then_resolution_time_discard() {
    let controller = PlayerId(0);
    let mut game = rav_game();
    let guildmage = game
        .put_on_battlefield(controller, "RAV-DIMIR-GUILDMAGE")
        .expect("Guildmage setup");
    let drawn = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Library)
        .expect("target draw setup");
    let discarded = game
        .add_card(PlayerId(1), "RAV-GLASS-GOLEM", Zone::Hand)
        .expect("target discard setup");
    game.grant_mana(controller, Color::Blue, 4)
        .expect("draw activation mana");
    game.grant_mana(controller, Color::Black, 4)
        .expect("discard activation mana");
    game.clear_event_log();

    game.activate_ability(controller, activation(guildmage, "target-player-draw"))
        .expect("sorcery-speed draw activation");
    game.pass_priority(controller).expect("controller passes");
    game.pass_priority(PlayerId(1))
        .expect("target player resolves draw ability");
    game.activate_ability(controller, activation(guildmage, "target-player-discard"))
        .expect("instant-speed discard activation");
    game.pass_priority(controller).expect("controller passes");
    game.pass_priority(PlayerId(1))
        .expect("target player resolves discard ability");

    println!("Dimir Guildmage trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == guildmage && *ability == "target-player-draw"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDiscarded { player, card }
            if *player == PlayerId(1) && *card == discarded
    )));
    game.validate_invariants()
        .expect("targeted ability trace preserves invariants");
}

#[test]
fn dimir_guildmage_draw_activation_rejects_nonmain_priority_without_paying() {
    let controller = PlayerId(0);
    let mut game = rav_game();
    let guildmage = game
        .put_on_battlefield(controller, "RAV-DIMIR-GUILDMAGE")
        .expect("Guildmage setup");
    let islands = (0..4)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-ISLAND")
                .expect("Island setup")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins at upkeep");
    for island in islands {
        game.activate_mana_ability(controller, island, Color::Blue)
            .expect("live blue mana");
    }
    let before_events = game.event_log.clone();
    assert_eq!(
        game.activate_ability(controller, activation(guildmage, "target-player-draw")),
        Err(RulesError::IllegalAction(
            "this activated ability is allowed only during your main phase with an empty stack"
        ))
    );
    assert_eq!(game.stack.len(), 0);
    assert_eq!(game.event_log, before_events, "rejection is atomic");
    assert_eq!(
        game.player(controller)
            .expect("controller exists")
            .mana_pool
            .amount(Color::Blue),
        4
    );
    game.validate_invariants()
        .expect("sorcery-speed rejection preserves invariants");
}
