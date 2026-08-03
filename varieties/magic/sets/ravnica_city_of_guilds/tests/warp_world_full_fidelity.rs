//! Event-log contract for Warp World's owner/library exchange.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Zone};
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
    .expect("RAV fixture builds")
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture begins");
    while game.step != Step::PrecombatMain {
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first stack pass");
    let second = game.priority;
    game.pass_priority(second).expect("second stack pass");
}

#[test]
fn warp_world_records_owner_shuffle_exact_reveal_count_and_simultaneous_permanent_return() {
    let controller = PlayerId(0);
    let mut game = rav_game();
    let warp = game
        .add_card(controller, "RAV-WARP-WORLD", Zone::Hand)
        .expect("Warp World setup");
    let original_creature = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("original creature setup");
    let mountains = (0..8)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-MOUNTAIN")
                .expect("mana land setup")
        })
        .collect::<Vec<_>>();
    for _ in 0..9 {
        game.add_card(controller, "RAV-FOREST", Zone::Library)
            .expect("permanent-only reveal fixture");
    }

    advance_to_precombat_main(&mut game);
    for mountain in mountains {
        game.activate_mana_ability(controller, mountain, Color::Red)
            .expect("red mana activation");
    }
    game.clear_event_log();
    game.cast_spell(
        controller,
        CastRequest {
            card: warp,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Warp World casts");
    resolve_top(&mut game);

    println!("warp_world_trace={:#?}", game.canonical_event_log());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled { player, cards } if *player == controller && *cards == 18
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardRevealed { player, .. } if *player == controller))
            .count(),
        9,
        "the reveal count is the original owner-permanent count"
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::CardMoved {
                    to: Zone::Library,
                    ..
                }
            ))
            .count(),
        9,
        "every original owned permanent crosses to its owner's library"
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::CardMoved {
                    to: Zone::Battlefield,
                    ..
                }
            ))
            .count(),
        9,
        "each revealed permanent returns through the shared entry batch"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Library } if *card == original_creature
    )));
    game.validate_invariants()
        .expect("Warp World owner/library exchange preserves invariants");
}
