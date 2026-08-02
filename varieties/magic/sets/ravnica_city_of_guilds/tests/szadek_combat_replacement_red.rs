//! Red regression for Szadek's combat-damage replacement.

use cardbench_magic_engine::{CounterKind, Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_damage_replacement_effect_bindings,
};

#[test]
fn szadek_replaces_player_combat_damage_with_mill_and_source_counters() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SZADEK")
        .expect("Szadek definition exists");

    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .expect("RAV replacement bindings register");
    let szadek = game
        .put_on_battlefield(PlayerId(0), "RAV-SZADEK")
        .expect("Szadek begins on the battlefield");
    for _ in 0..5 {
        game.add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
            .expect("opponent library fixture");
    }
    game.set_entered_turn_for_setup(szadek, 0)
        .expect("Szadek predates this turn");
    game.begin_game().expect("fixture starts");
    while game.step != Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to attackers");
    }
    game.declare_attackers(PlayerId(0), &[szadek])
        .expect("Szadek attacks");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("advance to blockers");
    game.declare_blockers(PlayerId(1), &[])
        .expect("no blockers");
    for _ in 0..4 {
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("advance through combat damage");
    }

    println!("szadek_red_event_log={:?}", game.canonical_event_log());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDamageReplacedWithMillAndCounters {
            source,
            player: PlayerId(1),
            amount: 5,
            ..
        } if *source == szadek
    )));
    assert_eq!(
        game.player(PlayerId(1)).expect("opponent exists").life,
        20,
        "the player-damage packet must be replaced, not dealt"
    );
    assert!(
        game.player(PlayerId(1))
            .expect("opponent exists")
            .library
            .is_empty(),
        "five cards must move from the damaged player's library"
    );
    assert_eq!(
        game.characteristics(szadek)
            .expect("Szadek remains live")
            .power,
        Some(10),
        "the source receives one +1/+1 counter per replaced damage"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced {
            source,
            card,
            counter: CounterKind::PlusOnePlusOne,
            amount: 5,
        } if *source == szadek && *card == szadek
    )));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPlayer { source, player, .. }
                if *source == szadek && *player == PlayerId(1)
        )),
        "the original player-damage receipt must not survive the replacement"
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "automatic replacement ordering remains outside this bounded slice"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"combat-player-damage-mill-and-counter-replacement"),
        "the replacement must be declared rather than approximated as ordinary damage"
    );
    game.validate_invariants()
        .expect("Szadek replacement trace preserves invariants");
}
