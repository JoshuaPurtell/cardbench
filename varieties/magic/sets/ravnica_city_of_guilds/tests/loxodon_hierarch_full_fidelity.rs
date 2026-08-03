//! Stack and regeneration contracts for Loxodon Hierarch.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone,
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
fn loxodon_hierarch_gains_life_then_sacrifices_for_controller_team_shields() {
    let mut game = game_with_rav_bindings();
    let ally = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("controlled creature setup");
    let opponent = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("opponent creature setup");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("opponent removal setup");
    let mountains = (0..3)
        .map(|_| {
            game.add_card(PlayerId(1), "RAV-MOUNTAIN", Zone::Battlefield)
                .expect("opponent mana setup")
        })
        .collect::<Vec<_>>();
    let hierarchy = game
        .add_card(PlayerId(0), "RAV-LOXODON-HIERARCH", Zone::Hand)
        .expect("Loxodon Hierarch setup");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("green mana");
    game.grant_mana(PlayerId(0), Color::White, 2)
        .expect("white mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: hierarchy,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Loxodon Hierarch casts");
    resolve_top(&mut game);
    resolve_top(&mut game);
    assert_eq!(game.players[0].life, 24, "entry trigger gains four life");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: hierarchy,
            ability_id: "sacrifice-source-regenerate-controller-creatures",
            sacrifice_sources: vec![hierarchy],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("source-sacrifice team-regeneration ability activates");
    assert_eq!(game.zone_of(hierarchy), Some(Zone::Graveyard));
    resolve_top(&mut game);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == hierarchy && *target == ally
    )));
    assert!(game.event_log.iter().all(|event| !matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == hierarchy && *target == opponent
    )));

    // The shield is a real one-shot replacement rather than an event-log-only
    // marker: the opponent deals lethal damage to the ally and it survives
    // tapped with its damage cleared.
    game.pass_priority(PlayerId(0))
        .expect("controller passes to opponent");
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(1), mountain, Color::Red)
            .expect("opponent produces Char mana");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(ally)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent casts Char at shielded creature");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(ally), Some(Zone::Battlefield));
    assert!(game.object(ally).expect("ally remains").tapped);
    assert_eq!(game.object(ally).expect("ally remains").damage, 0);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldUsed { source, target }
            if *source == hierarchy && *target == ally
    )));
    println!("Loxodon Hierarch trace: {:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("source-sacrifice team shields preserve invariants");
}
