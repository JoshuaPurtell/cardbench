//! Green regression for Caregiver's stack-backed targeted prevention shield.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn caregiver_sacrifices_source_and_prevents_only_next_player_damage() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV catalog constructs");
    let caregiver = game
        .put_on_battlefield(PlayerId(0), "RAV-CAREGIVER")
        .expect("Caregiver begins on the battlefield");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains begins on the battlefield");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain begins on the battlefield")
        })
        .collect::<Vec<_>>();
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("Char begins in hand");
    game.begin_game().expect("fixture begins game");

    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("Plains pays Caregiver activation");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: caregiver,
            ability_id: "prevent-one-damage",
            sacrifice_sources: vec![caregiver],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(0))],
        },
    )
    .expect("Caregiver activation resolves through the stack");
    assert_eq!(game.zone_of(caregiver), Some(Zone::Graveyard));
    resolve_top(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageShieldCreated {
            source,
            target: Target::Player(PlayerId(0)),
            amount: 1,
        } if *source == caregiver
    )));

    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("Mountain produces Char mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Player(PlayerId(0))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char targets the protected player");
    resolve_top(&mut game);

    assert_eq!(
        game.players[0].life, 15,
        "one of Char's six damage is prevented"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Player(PlayerId(0)),
            amount: 1,
        } if *source == char
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer {
            source,
            player: PlayerId(0),
            amount: 3,
        } if *source == char
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer {
            source,
            player: PlayerId(0),
            amount: 2,
        } if *source == char
    )));
    game.validate_invariants()
        .expect("targeted prevention state remains invariant-valid");
}
