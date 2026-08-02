//! Event-log contract for Tidewater Minion's two stack-backed abilities.

use cardbench_magic_engine::{
    AbilityActivation, Color, Game, GameEvent, Keyword, PlayerId, Target,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
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

fn pass_stack_object(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn minion_untaps_a_target_permanent_and_loses_defender_after_blue_payment() {
    let mut game = game_with_rav_bindings();
    let minion = game
        .put_on_battlefield(PlayerId(0), "RAV-TIDEWATER-MINION")
        .expect("Minion setup");
    let target_island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("target permanent setup");
    let payment_island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("payment source setup");
    game.set_entered_turn_for_setup(minion, 0)
        .expect("Minion entered on an earlier turn");
    game.begin_game().expect("game begins");
    game.clear_event_log();

    game.activate_mana_ability(PlayerId(0), target_island, Color::Blue)
        .expect("target Island taps for setup mana");
    game.activate_mana_ability(PlayerId(0), payment_island, Color::Blue)
        .expect("second Island provides the defender-removal payment");
    assert!(game.object(target_island).expect("target exists").tapped);
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: minion,
            ability_id: "tap-untap-target-permanent",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target_island)],
        },
    )
    .expect("Minion targets an arbitrary permanent");
    pass_stack_object(&mut game);
    assert!(!game.object(target_island).expect("target exists").tapped);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentUntapped { source, card }
            if *source == minion && *card == target_island
    )));

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: minion,
            ability_id: "blue-lose-defender",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Minion pays Blue to lose Defender");
    pass_stack_object(&mut game);
    println!(
        "tidewater_minion_event_log={:#?}",
        game.canonical_event_log()
    );

    assert!(game.object(minion).expect("Minion exists").tapped);
    assert!(
        !game
            .characteristics(minion)
            .expect("Minion characteristics")
            .keywords
            .contains(&Keyword::Defender),
        "the second ability removes Defender only through the current turn"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == minion && *ability == "blue-lose-defender"
    )));
    game.validate_invariants()
        .expect("Tidewater Minion trace preserves invariants");
}
