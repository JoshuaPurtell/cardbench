//! Red discovery contract for the source lane's unmerged Vinelasher landfall.

use cardbench_magic_engine::{Effect, Game, GameEvent, PlayerId, Step, TriggerCondition, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_triggers() -> Game {
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

fn advance_to_first_main(game: &mut Game) {
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player advances the early turn");
        game.pass_priority(PlayerId(1))
            .expect("opponent advances the early turn");
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

fn advance_to_second_players_first_main(game: &mut Game) {
    game.pass_priority(PlayerId(0))
        .expect("first player passes precombat main");
    game.pass_priority(PlayerId(1))
        .expect("second player passes precombat main");
    game.pass_priority(PlayerId(0))
        .expect("first player passes beginning of combat");
    game.pass_priority(PlayerId(1))
        .expect("second player passes beginning of combat");
    assert_eq!(game.step, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[])
        .expect("first player declares no attackers");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("first player finishes the turn");
        game.pass_priority(PlayerId(1))
            .expect("second player finishes the first turn");
    }
    assert_eq!(game.active_player, PlayerId(1));
    assert_eq!(game.step, Step::Upkeep);
    game.pass_priority(PlayerId(1))
        .expect("second player passes upkeep");
    game.pass_priority(PlayerId(0))
        .expect("first player passes second-player upkeep");
    assert_eq!(game.step, Step::Draw);
    game.draw_card(PlayerId(1), None)
        .expect("second player's ordinary draw resolves");
    game.pass_priority(PlayerId(1))
        .expect("second player passes draw priority");
    game.pass_priority(PlayerId(0))
        .expect("first player passes draw priority");
    assert_eq!(game.step, Step::PrecombatMain);
}

#[test]
fn vinelasher_kudzu_registers_its_controller_land_entry_counter_trigger() {
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-VINELASHER-KUDZU")
        .expect("Vinelasher Kudzu landfall trigger exists");

    assert_eq!(
        binding.ability.condition,
        TriggerCondition::ControlledLandEntersBattlefield
    );
    assert_eq!(binding.ability.targets, []);
    assert_eq!(binding.ability.effects, [Effect::AddPlusOneCounterToSource]);
}

#[test]
fn vinelasher_kudzu_stacks_and_resolves_one_counter_for_its_controllers_land_play() {
    let mut game = game_with_rav_triggers();
    let kudzu = game
        .put_on_battlefield(PlayerId(0), "RAV-VINELASHER-KUDZU")
        .expect("Kudzu setup");
    game.set_entered_turn_for_setup(kudzu, 0)
        .expect("Kudzu predates the first turn");
    let _initial_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("initial Forest setup");
    let entering_forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Hand)
        .expect("land-play setup");

    game.begin_game().expect("fixture starts");
    advance_to_first_main(&mut game);
    game.clear_event_log();

    game.play_land(PlayerId(0), entering_forest)
        .expect("controller plays a land");
    assert_eq!(game.stack.len(), 1, "landfall uses a stack object");
    game.pass_priority(PlayerId(0))
        .expect("controller passes the trigger");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves the trigger");

    println!(
        "Vinelasher Kudzu landfall trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.object(kudzu)
            .expect("Kudzu remains on battlefield")
            .counters
            .get("+1/+1"),
        Some(&1)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced { source, card, counter: "+1/+1", amount: 1 }
            if *source == kudzu && *card == kudzu
    )));
    game.validate_invariants()
        .expect("landfall counter trace preserves invariants");
}

#[test]
fn vinelasher_kudzu_does_not_trigger_from_an_opponents_land_play() {
    let mut game = game_with_rav_triggers();
    let kudzu = game
        .put_on_battlefield(PlayerId(0), "RAV-VINELASHER-KUDZU")
        .expect("Kudzu setup");
    game.set_entered_turn_for_setup(kudzu, 0)
        .expect("Kudzu predates the first turn");
    let opponent_forest = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Hand)
        .expect("opponent land-play setup");
    let _draw_card = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Library)
        .expect("second-player draw fixture");

    game.begin_game().expect("fixture starts");
    advance_to_first_main(&mut game);
    advance_to_second_players_first_main(&mut game);
    game.clear_event_log();

    game.play_land(PlayerId(1), opponent_forest)
        .expect("opponent plays a land");

    assert!(
        game.stack.is_empty(),
        "opponent land must not stack Kudzu landfall"
    );
    assert!(game.event_log.iter().all(|event| !matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == kudzu && *ability == "controller-landfall-plus-one-counter"
    )));
    assert!(
        !game
            .object(kudzu)
            .expect("Kudzu remains on battlefield")
            .counters
            .contains_key("+1/+1"),
        "an opponent land cannot create a Kudzu counter"
    );
    game.validate_invariants()
        .expect("opponent land-entry no-op remains invariant-valid");
}
