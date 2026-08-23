//! Live-game contract for both sorcery-speed Dimir Guildmage activations.

use cardbench_magic_engine::{
    AbilityActivation, Color, DecisionSelection, Game, GameEvent, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn guildmage_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture constructs")
}

fn advance_to_first_main(game: &mut Game) {
    game.begin_game().expect("fixture begins game");
    while game.step != Step::PrecombatMain {
        let player = game.priority;
        game.pass_priority(player)
            .expect("fixture priority passes toward main");
    }
    assert_eq!(game.priority, PlayerId(0));
}

#[test]
fn dimir_guildmage_discard_uses_a_live_main_phase_private_recipient_choice() {
    let controller = PlayerId(0);
    let recipient = PlayerId(1);
    let mut game = guildmage_game();
    let guildmage = game
        .put_on_battlefield(controller, "RAV-DIMIR-GUILDMAGE")
        .expect("Guildmage setup");
    let discarded = game
        .add_card(recipient, "RAV-WATCHWOLF", Zone::Hand)
        .expect("recipient hand setup");
    game.add_card(controller, "RAV-GLASS-GOLEM", Zone::Library)
        .expect("draw-step fixture setup");
    let swamps = (0..4)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-SWAMP")
                .expect("Swamp setup")
        })
        .collect::<Vec<_>>();

    advance_to_first_main(&mut game);
    for swamp in swamps {
        game.activate_mana_ability(controller, swamp, Color::Black)
            .expect("live black mana");
    }
    game.activate_ability(
        controller,
        AbilityActivation {
            source: guildmage,
            ability_id: "target-player-discard",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(recipient)],
        },
    )
    .expect("discard activation is legal in the controller main phase");
    game.pass_priority(controller).expect("controller passes");
    game.pass_priority(recipient)
        .expect("recipient opens private discard decision");
    let decision = game
        .view_for_player(recipient)
        .expect("recipient view")
        .pending_decision
        .expect("recipient-private discard decision opens");
    assert!(
        game.view_for_player(controller)
            .expect("controller view")
            .pending_decision
            .is_none(),
        "the controller cannot see the recipient's private hand decision"
    );
    game.submit_decision(
        recipient,
        decision.id,
        DecisionSelection::Objects(vec![discarded]),
    )
    .expect("recipient selects exactly one current hand card");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-DIMIR-GUILDMAGE"));
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened { player, visibility, .. }
            if *player == recipient && *visibility == cardbench_magic_engine::DecisionVisibility::Private
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDiscarded { player, card }
            if *player == recipient && *card == discarded
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == guildmage && *ability == "target-player-discard"
    )));
    game.validate_invariants()
        .expect("live sorcery-speed discard preserves invariants");
    eprintln!(
        "Dimir Guildmage discard trace={:?}",
        game.canonical_event_log()
    );
}
