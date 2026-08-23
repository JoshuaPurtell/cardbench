//! Event-log contract for Flow of Ideas' typed Island count.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Zone};
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
    .expect("RAV game builds")
}

#[test]
fn flow_of_ideas_draws_once_per_controller_island_not_opponent_islands() {
    let mut game = game_with_rav_bindings();
    let flow = game
        .add_card(PlayerId(0), "RAV-FLOW-OF-IDEAS", Zone::Hand)
        .expect("Flow setup");
    let bottom = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("bottom card setup");
    let middle = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("middle card setup");
    let top = game
        .add_card(PlayerId(0), "RAV-SWAMP", Zone::Library)
        .expect("top card setup");
    let islands = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("controller Island")
        })
        .collect::<Vec<_>>();
    let mountains = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("generic payment source")
        })
        .collect::<Vec<_>>();
    game.put_on_battlefield(PlayerId(1), "RAV-ISLAND")
        .expect("opponent Island does not contribute");
    game.begin_game().expect("game begins");
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first phase pass");
        let second = game.priority;
        game.pass_priority(second).expect("second phase pass");
    }
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("blue payment mana");
    }
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("generic payment mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: flow,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Flow casts with 5U");
    game.pass_priority(PlayerId(0))
        .expect("caster passes priority");
    game.pass_priority(PlayerId(1)).expect("Flow resolves");

    println!("flow_of_ideas_event_log={:#?}", game.canonical_event_log());
    assert_eq!(game.zone_of(top), Some(Zone::Hand));
    assert_eq!(game.zone_of(middle), Some(Zone::Hand));
    assert_eq!(game.zone_of(bottom), Some(Zone::Library));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == top || *card == middle))
            .count(),
        2
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellResolved { card } if *card == flow
    )));
    assert_eq!(game.zone_of(flow), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("Flow trace preserves typed land and stack invariants");
}

#[test]
fn flow_of_ideas_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-FLOW-OF-IDEAS"));
}
