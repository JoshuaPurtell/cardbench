//! Event and lifecycle contract for Roofstalker Wight's self-Flying ability.

use cardbench_magic_engine::{
    AbilityActivation, Color, Game, GameEvent, Keyword, Layer, ManaCost, PlayerId, Step,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture constructs")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

fn advance_past_cleanup(game: &mut Game) {
    let turn = game.turn;
    while game.turn == turn {
        if game.step == Step::DeclareAttackers
            && game.declare_attackers(game.active_player, &[]).is_ok()
        {
            continue;
        }
        if game.step == Step::DeclareBlockers && game.declare_blockers(game.priority, &[]).is_ok() {
            continue;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("empty priority windows advance the turn");
    }
}

#[test]
fn roofstalker_wight_gains_flying_only_for_the_resolving_turn() {
    let controller = PlayerId(0);
    let mut game = game_with_rav_bindings();
    let wight = game
        .put_on_battlefield(controller, "RAV-ROOFSTALKER-WIGHT")
        .expect("Wight begins on battlefield");
    let island = game
        .put_on_battlefield(controller, "RAV-ISLAND")
        .expect("first Island pays activation");
    let second_island = game
        .put_on_battlefield(controller, "RAV-ISLAND")
        .expect("second Island pays activation generic component");
    game.begin_game().expect("fixture begins");
    game.clear_event_log();

    assert!(
        !game
            .characteristics(wight)
            .expect("Wight has characteristics")
            .keywords
            .contains(&Keyword::Flying)
    );
    game.activate_mana_ability(controller, island, Color::Blue)
        .expect("Island supplies the Blue activation payment");
    game.activate_mana_ability(controller, second_island, Color::Blue)
        .expect("second Island supplies the generic activation payment");
    game.activate_ability(
        controller,
        AbilityActivation {
            source: wight,
            ability_id: "blue-gain-flying",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("target-free self ability enters the stack");
    resolve_top(&mut game);

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-ROOFSTALKER-WIGHT"));
    assert!(
        game.characteristics(wight)
            .expect("resolving source remains live")
            .keywords
            .contains(&Keyword::Flying)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityManaPaid { source, ability, mana_cost, .. }
            if *source == wight
                && *ability == "blue-gain-flying"
                && *mana_cost == ManaCost::with_colors(1, [Color::Blue])
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, layer }
            if *source == wight && *target == wight && *layer == Layer::Ability
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == wight && *ability == "blue-gain-flying"
    )));
    println!(
        "roofstalker_wight_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("ability stack and continuous-effect lifecycle are valid");

    advance_past_cleanup(&mut game);
    assert!(
        !game
            .characteristics(wight)
            .expect("Wight remains live after cleanup")
            .keywords
            .contains(&Keyword::Flying),
        "the source-relative keyword grant must expire at cleanup"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source, target, layer }
            if *source == wight && *target == wight && *layer == Layer::Ability
    )));
    game.validate_invariants()
        .expect("cleanup expiry preserves the lifecycle invariant");
}
