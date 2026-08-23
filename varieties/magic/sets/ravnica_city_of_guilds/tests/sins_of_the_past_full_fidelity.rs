//! Full-fidelity contract for Sins of the Past's graveyard cast permission.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn sins_grants_a_mana_free_graveyard_cast_that_exiles_on_resolution() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-SINS-OF-THE-PAST"));
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let sins = game
        .add_card(PlayerId(0), "RAV-SINS-OF-THE-PAST", Zone::Hand)
        .expect("Sins is executable");
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Graveyard)
        .expect("Char is an eligible target");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("Char target enters");
    let swamps = (0..6)
        .map(|_| {
            game.add_card(PlayerId(0), "RAV-SWAMP", Zone::Battlefield)
                .expect("Swamp")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("first pass");
        game.pass_priority(PlayerId(1)).expect("second pass");
    }
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("Swamp pays Sins");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: sins,
            targets: vec![Target::Permanent(char)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Sins casts");
    game.pass_priority(PlayerId(0)).expect("offer Sins");
    game.pass_priority(PlayerId(1)).expect("Sins resolves");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("permission cast is mana-free");
    game.pass_priority(PlayerId(0)).expect("offer Char");
    game.pass_priority(PlayerId(1)).expect("Char resolves");
    println!("Sins trace: {:?}", game.canonical_event_log());
    assert_eq!(game.zone_of(char), Some(Zone::Exile));
    assert!(game.event_log.iter().any(
        |event| matches!(event, GameEvent::SpellCastFromGraveyard { card, .. } if *card == char)
    ));
    game.validate_invariants()
        .expect("permission lifecycle is valid");
}

#[test]
fn permission_cast_is_exiled_when_countered_by_a_spell() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let sins = game
        .add_card(PlayerId(0), "RAV-SINS-OF-THE-PAST", Zone::Hand)
        .expect("Sins is executable");
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Graveyard)
        .expect("Char is an eligible target");
    let muddle = game
        .add_card(PlayerId(1), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)
        .expect("Muddle is executable");
    game.grant_mana(PlayerId(0), Color::Black, 6)
        .expect("Sins mana is available");
    game.grant_mana(PlayerId(1), Color::Blue, 2)
        .expect("Muddle mana is available");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: sins,
            targets: vec![Target::Permanent(char)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Sins casts");
    game.pass_priority(PlayerId(0)).expect("offer Sins");
    game.pass_priority(PlayerId(1)).expect("resolve Sins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("permission cast succeeds");
    game.pass_priority(PlayerId(0)).expect("offer Char");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: muddle,
            targets: vec![Target::Spell(char)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Muddle counters the permission cast");
    game.pass_priority(PlayerId(1)).expect("offer Muddle");
    let result = game.pass_priority(PlayerId(0));
    println!(
        "Sins countered-cast result: {result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(result.is_ok(), "Muddle resolution must preserve invariants");
    assert_eq!(game.zone_of(char), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::SpellCountered { card, source } if *card == char && *source == muddle)
    }));
    game.validate_invariants()
        .expect("countered permission cast preserves invariants");
}
