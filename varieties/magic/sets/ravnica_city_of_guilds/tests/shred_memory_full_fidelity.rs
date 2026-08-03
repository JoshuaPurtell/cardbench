//! Full-fidelity target-group regression for Shred Memory.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
#[allow(clippy::too_many_lines)] // One transcript covers target range, membership, and zero-target resolution.
fn shred_memory_exiles_a_variable_same_graveyard_target_group_or_zero_targets() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let first = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Graveyard)
        .expect("first opponent graveyard card");
    let second = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Graveyard)
        .expect("second opponent graveyard card");
    let third = game
        .add_card(PlayerId(1), "RAV-SWAMP", Zone::Graveyard)
        .expect("third opponent graveyard card");
    let fourth = game
        .add_card(PlayerId(1), "RAV-MOUNTAIN", Zone::Graveyard)
        .expect("fourth opponent graveyard card");
    let fifth = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Graveyard)
        .expect("fifth opponent graveyard card");
    let own_graveyard_card = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Graveyard)
        .expect("controller graveyard card");
    let shred = game
        .add_card(PlayerId(0), "RAV-SHRED-MEMORY", Zone::Hand)
        .expect("first Shred Memory setup");
    let zero_target_shred = game
        .add_card(PlayerId(0), "RAV-SHRED-MEMORY", Zone::Hand)
        .expect("zero-target Shred Memory setup");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("two casts worth of pregame mana");

    let over_range = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: shred,
            targets: vec![
                Target::Permanent(first),
                Target::Permanent(second),
                Target::Permanent(third),
                Target::Permanent(fourth),
                Target::Permanent(fifth),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    assert!(
        over_range.is_err(),
        "target groups cannot exceed four cards"
    );
    let duplicate = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: shred,
            targets: vec![Target::Permanent(first), Target::Permanent(first)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    assert!(duplicate.is_err(), "target groups require distinct cards");
    let rejected = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: shred,
            targets: vec![
                Target::Permanent(first),
                Target::Permanent(own_graveyard_card),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    assert!(rejected.is_err(), "targets cannot span two graveyards");
    assert_eq!(game.zone_of(shred), Some(Zone::Hand));

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: shred,
            targets: vec![
                Target::Permanent(first),
                Target::Permanent(second),
                Target::Permanent(third),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("three same-graveyard targets are legal");
    resolve_top(&mut game);
    for card in [first, second, third] {
        assert_eq!(game.zone_of(card), Some(Zone::Exile));
    }
    assert_eq!(game.zone_of(own_graveyard_card), Some(Zone::Graveyard));

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: zero_target_shred,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero targets are legal for an up-to-four effect");
    resolve_top(&mut game);

    println!("Shred Memory trace: {:?}", game.canonical_event_log());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-SHRED-MEMORY"));
    assert_eq!(game.zone_of(zero_target_shred), Some(Zone::Graveyard));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Exile } if [first, second, third].contains(card)))
            .count(),
        3,
        "every chosen target has an ordinary exile receipt"
    );
    game.validate_invariants()
        .expect("variable target-group stack state remains invariant-valid");
}

#[test]
fn shred_memory_skips_a_group_member_that_leaves_before_resolution() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV catalog and ability bindings build");
    let saved = game
        .add_card(PlayerId(0), "RAV-FISTS-OF-IRONWOOD", Zone::Graveyard)
        .expect("first controller graveyard card");
    let exiled = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Graveyard)
        .expect("second controller graveyard card");
    let shred = game
        .add_card(PlayerId(0), "RAV-SHRED-MEMORY", Zone::Hand)
        .expect("Shred Memory setup");
    let dowsing_shaman = game
        .add_card(PlayerId(0), "RAV-DOWSING-SHAMAN", Zone::Battlefield)
        .expect("instant-speed graveyard response setup");
    game.set_entered_turn_for_setup(dowsing_shaman, 0)
        .expect("Dowsing Shaman is long-controlled");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("black mana for Shred and activation generic cost");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("green mana for Dowsing Shaman");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: shred,
            targets: vec![Target::Permanent(saved), Target::Permanent(exiled)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Shred targets two cards from one graveyard");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: dowsing_shaman,
            ability_id: "return-target-enchantment-from-graveyard",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(saved)],
        },
    )
    .expect("instant-speed response returns one retained target");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(saved), Some(Zone::Hand));
    resolve_top(&mut game);

    println!(
        "Shred Memory partial-resolution trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(saved), Some(Zone::Hand));
    assert_eq!(game.zone_of(exiled), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TargetInstructionSkipped { card, target, .. }
            if *card == shred && *target == Target::Permanent(saved)
    )));
    game.validate_invariants()
        .expect("partially resolved target group remains invariant-valid");
}
