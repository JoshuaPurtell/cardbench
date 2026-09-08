//! Red regression for Disembowel's missing chosen-X cast boundary.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, ManaPaymentSelection, PlayerId, Target, Zone,
};
use cardbench_magic_rav::card_definitions;

#[test]
fn disembowel_pays_explicit_x_and_destroys_a_creature_with_exact_mana_value() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let spell = game
        .add_card(PlayerId(0), "RAV-DISEMBOWEL", Zone::Hand)
        .expect("Disembowel setup");
    let target = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("target setup");
    game.grant_mana(PlayerId(0), Color::Black, 3)
        .expect("black mana for X=2 plus the colored symbol");

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![Color::Black, Color::Black],
            hybrid: vec![],
        },
    )
    .expect("chosen X pays Disembowel");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell resolves");

    println!(
        "Disembowel chosen-X trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellManaPaid { player, card, colors }
            if *player == PlayerId(0)
                && *card == spell
                && colors == &vec![Color::Black, Color::Black, Color::Black]
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card } if *source == spell && *card == target
    )));
    game.validate_invariants()
        .expect("chosen-X resolution preserves invariant state");
}

#[test]
fn disembowel_rejects_overpayment_as_well_as_underpayment() {
    let mut game = Game::new(card_definitions(), 2).unwrap();
    let spell = game.add_card(PlayerId(0), "RAV-DISEMBOWEL", Zone::Hand).unwrap();
    let target = game.add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield).unwrap();
    game.grant_mana(PlayerId(0), Color::Black, 4).unwrap();
    let before = game.canonical_event_log();
    assert!(game.cast_spell_with_x(PlayerId(0), CastRequest {
        card: spell, targets: vec![Target::Permanent(target)], convoke: vec![], payment_mana_abilities: vec![],
    }, 3, ManaPaymentSelection { generic: vec![Color::Black; 3], hybrid: vec![] }).is_err());
    assert_eq!(game.canonical_event_log(), before);
    assert_eq!(game.players[0].mana_pool.amount(Color::Black), 4);
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
}

#[test]
fn disembowel_rechecks_exact_mana_value_after_a_copy_change() {
    let mut game = Game::new(card_definitions(), 2).unwrap();
    let spell = game.add_card(PlayerId(0), "RAV-DISEMBOWEL", Zone::Hand).unwrap();
    let target = game.add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield).unwrap();
    let copy_source = game.add_card(PlayerId(1), "RAV-GOLIATH-SPIDER", Zone::Battlefield).unwrap();
    game.grant_mana(PlayerId(0), Color::Black, 3).unwrap();
    game.cast_spell_with_x(PlayerId(0), CastRequest {
        card: spell, targets: vec![Target::Permanent(target)], convoke: vec![], payment_mana_abilities: vec![],
    }, 2, ManaPaymentSelection { generic: vec![Color::Black; 2], hybrid: vec![] }).unwrap();
    // Unit-only copy seam changes characteristics without changing incarnation.
    game.copy_permanent(target, copy_source).unwrap();
    game.pass_priority(PlayerId(0)).unwrap();
    game.pass_priority(PlayerId(1)).unwrap();
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(event,
        GameEvent::SpellCounteredByRules { card } if *card == spell)));
    game.validate_invariants().unwrap();
}

#[test]
fn an_x_damage_quantity_does_not_impose_disembowels_target_restriction() {
    let mut game = Game::new(card_definitions(), 2).unwrap();
    let spell = game.add_card(PlayerId(0), "RAV-BRIGHTFLAME", Zone::Hand).unwrap();
    let target = game.add_card(PlayerId(1), "RAV-GOLIATH-SPIDER", Zone::Battlefield).unwrap();
    game.grant_mana(PlayerId(0), Color::Red, 3).unwrap();
    game.grant_mana(PlayerId(0), Color::White, 2).unwrap();
    game.cast_spell_with_x(PlayerId(0), CastRequest {
        card: spell, targets: vec![Target::Permanent(target)], convoke: vec![], payment_mana_abilities: vec![],
    }, 1, ManaPaymentSelection { generic: vec![Color::Red], hybrid: vec![] }).unwrap();
    game.pass_priority(PlayerId(0)).unwrap();
    game.pass_priority(PlayerId(1)).unwrap();
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert!(!game.event_log.iter().any(|event| matches!(event,
        GameEvent::SpellCounteredByRules { card } if *card == spell)));
    game.validate_invariants().unwrap();
}

#[test]
fn disembowel_rejects_a_target_above_the_selected_x_before_any_cast_mutation() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let spell = game
        .add_card(PlayerId(0), "RAV-DISEMBOWEL", Zone::Hand)
        .expect("Disembowel setup");
    let target = game
        .add_card(PlayerId(1), "RAV-GOLIATH-SPIDER", Zone::Battlefield)
        .expect("over-bound target setup");
    game.grant_mana(PlayerId(0), Color::Black, 3)
        .expect("black mana is available before rejected cast");
    let events_before = game.event_log.clone();

    let result = game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![Color::Black, Color::Black],
            hybrid: vec![],
        },
    );

    assert!(result.is_err(), "mana value six must exceed selected X two");
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(game.players[0].mana_pool.amount(Color::Black), 3);
    assert_eq!(game.event_log, events_before);
    assert!(game.stack.is_empty());
    game.validate_invariants()
        .expect("rejected chosen-X cast preserves invariant state");
}

#[test]
fn disembowel_cannot_silently_default_x_to_zero_through_the_ordinary_cast_api() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let spell = game
        .add_card(PlayerId(0), "RAV-DISEMBOWEL", Zone::Hand)
        .expect("Disembowel setup");
    let target = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("target setup");
    game.grant_mana(PlayerId(0), Color::Black, 1)
        .expect("printed colored symbol is available");
    let events_before = game.event_log.clone();

    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );

    assert!(
        result.is_err(),
        "ordinary cast must require a declared X value"
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert_eq!(game.players[0].mana_pool.amount(Color::Black), 1);
    assert_eq!(game.event_log, events_before);
    assert!(game.stack.is_empty());
}
