//! Event-log contracts for Terraformer's controller-land type activation.

use cardbench_magic_engine::{
    AbilityActivation, BasicLandType, Color, Game, GameEvent, Layer, PlayerId, Target,
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

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn terraformer_replaces_only_controller_land_types_and_intrinsic_mana_through_end_of_turn() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game_with_rav_bindings();
    let terraformer = game
        .put_on_battlefield(controller, "RAV-TERRAFORMER")
        .expect("Terraformer setup");
    let controlled_plains = game
        .put_on_battlefield(controller, "RAV-PLAINS")
        .expect("controlled land setup");
    let payment_island = game
        .put_on_battlefield(controller, "RAV-ISLAND")
        .expect("payment land setup");
    let opponent_island = game
        .put_on_battlefield(opponent, "RAV-ISLAND")
        .expect("opponent land setup");

    game.begin_game().expect("game begins");
    game.clear_event_log();
    game.activate_mana_ability(controller, payment_island, Color::Blue)
        .expect("one generic payment mana");
    game.activate_ability(
        controller,
        AbilityActivation {
            source: terraformer,
            ability_id: "choose-controller-land-basic-type",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::BasicLandType(BasicLandType::Forest)],
        },
    )
    .expect("controller chooses Forest while activating Terraformer");
    resolve_top(&mut game);

    assert_eq!(
        game.basic_land_type(controlled_plains),
        Ok(Some(BasicLandType::Forest)),
        "the controller's former Plains has the selected current type"
    );
    assert_eq!(
        game.basic_land_type(payment_island),
        Ok(Some(BasicLandType::Forest)),
        "every controlled land receives an independent layer-four effect"
    );
    assert_eq!(
        game.basic_land_type(opponent_island),
        Ok(Some(BasicLandType::Island)),
        "opponent lands are not selected by a controller-scoped effect"
    );
    game.activate_mana_ability(controller, controlled_plains, Color::Green)
        .expect("the chosen basic type grants its intrinsic green mana ability");
    assert_eq!(
        game.player(controller)
            .expect("controller remains")
            .mana_pool
            .amount(Color::Green),
        1
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, layer }
            if *source == terraformer && *target == controlled_plains && *layer == Layer::Type
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == terraformer && *ability == "choose-controller-land-basic-type"
    )));
    println!("terraformer_event_log={:#?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Terraformer type and intrinsic-mana trace preserves invariants");
}

#[test]
fn terraformer_rejects_missing_or_nonchoice_activation_atomically() {
    let controller = PlayerId(0);
    let mut game = game_with_rav_bindings();
    let terraformer = game
        .put_on_battlefield(controller, "RAV-TERRAFORMER")
        .expect("Terraformer setup");
    let island = game
        .put_on_battlefield(controller, "RAV-ISLAND")
        .expect("payment land setup");
    game.begin_game().expect("game begins");
    game.activate_mana_ability(controller, island, Color::Blue)
        .expect("one generic payment mana");
    let before_events = game.event_log.clone();

    let error = game
        .activate_ability(
            controller,
            AbilityActivation {
                source: terraformer,
                ability_id: "choose-controller-land-basic-type",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        )
        .expect_err("the choice cannot be omitted or inferred");
    assert!(error.to_string().contains("basic-land-type choice"));
    assert_eq!(
        game.event_log, before_events,
        "invalid choice writes no receipt"
    );
    assert_eq!(
        game.stack.len(),
        0,
        "invalid choice never reaches the stack"
    );
    assert_eq!(
        game.player(controller)
            .expect("controller remains")
            .mana_pool
            .amount(Color::Blue),
        1,
        "invalid choice cannot consume mana"
    );
    game.validate_invariants()
        .expect("rejected Terraformer choice preserves invariants");
}
