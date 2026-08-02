//! Full-fidelity contract for Thoughtpicker Witch's private library choice.

use cardbench_magic_engine::{
    AbilityActivation, Color, Game, GameEvent, PlayerId, PolicyAction, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_witch() -> (
    Game,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let witch = game
        .put_on_battlefield(PlayerId(0), "RAV-THOUGHTPICKER-WITCH")
        .expect("Witch enters");
    let fodder = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled sacrifice creature enters");
    let second = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
        .expect("opponent second library card exists during setup");
    let top = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Library)
        .expect("opponent top library card exists during setup");
    for card in [witch, fodder] {
        game.set_entered_turn_for_setup(card, 0)
            .expect("fixture has prior-turn provenance");
    }
    game.begin_game().expect("game starts");
    (game, witch, fodder, second, top)
}

#[test]
fn witch_keeps_ability_on_stack_for_a_controller_private_opponent_library_choice() {
    let (mut game, witch, fodder, second, top) = game_with_witch();
    game.add_mana_from_action(PlayerId(0), Color::Black, 1)
        .expect("one generic ability mana is available");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: witch,
            ability_id: "sacrifice-creature-private-opponent-top-two-exile",
            sacrifice_sources: vec![fodder],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Witch activation reaches the stack");
    assert_eq!(game.zone_of(fodder), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1);

    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent opens the suspended private choice");
    assert_eq!(
        game.stack.len(),
        1,
        "the ability remains live while choosing"
    );
    let controller_choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .private_opponent_library_choice
        .expect("controller sees the candidate identities");
    assert_eq!(controller_choice.source, witch);
    assert_eq!(
        controller_choice
            .cards
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![top, second],
        "the candidate snapshot is top-first and preserves library order"
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .private_opponent_library_choice
            .is_none(),
        "the target opponent cannot inspect the private choice"
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::CardsLookedAt { .. })),
        "the public event log must not reveal cards from an opponent library"
    );
    assert!(
        game.pass_priority(PlayerId(0)).is_err(),
        "no priority action can interleave with the private choice"
    );

    game.submit_policy_move(
        PlayerId(0),
        "thoughtpicker-private-choice-contract",
        PolicyAction::ChoosePrivateOpponentLibraryCardToExile {
            source: witch,
            ability: "sacrifice-creature-private-opponent-top-two-exile",
            selected: Some(second),
        },
    )
    .expect("controller completes the mandatory choice");

    println!(
        "Thoughtpicker Witch event log: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(second), Some(Zone::Exile));
    assert_eq!(game.zone_of(top), Some(Zone::Library));
    assert_eq!(
        game.player(PlayerId(1)).expect("opponent remains").library,
        vec![top],
        "the unchosen card remains in its original order"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PrivateOpponentLibraryChoiceOpened {
            controller,
            source,
            ability,
            opponent,
            count,
            ..
        } if *controller == PlayerId(0)
            && *source == witch
            && *ability == "sacrifice-creature-private-opponent-top-two-exile"
            && *opponent == PlayerId(1)
            && *count == 2
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == witch && *ability == "sacrifice-creature-private-opponent-top-two-exile"
    )));
    game.validate_invariants()
        .expect("private choice trace preserves every state-machine invariant");
}

#[test]
fn witch_rejects_self_target_before_paying_its_sacrifice_cost() {
    let (mut game, witch, fodder, _, _) = game_with_witch();
    game.add_mana_from_action(PlayerId(0), Color::Black, 1)
        .expect("one generic ability mana is available");
    let event_log = game.event_log.clone();
    let result = game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: witch,
            ability_id: "sacrifice-creature-private-opponent-top-two-exile",
            sacrifice_sources: vec![fodder],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(0))],
        },
    );
    assert!(result.is_err(), "the target must be a living opponent");
    assert_eq!(game.zone_of(fodder), Some(Zone::Battlefield));
    assert!(game.stack.is_empty());
    assert_eq!(game.event_log, event_log, "rejection is atomic");
    game.validate_invariants()
        .expect("rejected target preserves invariants");
}
