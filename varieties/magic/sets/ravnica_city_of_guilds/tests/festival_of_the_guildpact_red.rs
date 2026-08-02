//! Red discovery contract for Festival of the Guildpact's chosen-X prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, ManaPaymentSelection, PlayerId,
    Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn festival_has_its_exact_chosen_x_prevention_and_draw_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FESTIVAL-OF-THE-GUILDPACT")
        .expect("Festival of the Guildpact definition exists");
    assert_eq!(definition.name, "Festival of the Guildpact");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"chosen-x-controller-damage-prevention-and-draw")
    );
}

#[test]
fn festival_prevents_the_chosen_amount_then_draws_before_later_damage() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let festival = game
        .add_card(PlayerId(0), "RAV-FESTIVAL-OF-THE-GUILDPACT", Zone::Hand)
        .expect("Festival of the Guildpact is in hand");
    let drawn_card = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("draw card is in controller library");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("later damage source is in opponent hand");
    game.grant_mana(PlayerId(0), Color::White, 3)
        .expect("{X}{W} payment exists for X=2");
    game.grant_mana(PlayerId(1), Color::Red, 3)
        .expect("Char payment exists");

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: festival,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![Color::White, Color::White],
            hybrid: vec![],
        },
    )
    .expect("Festival with X=2 casts");
    game.pass_priority(PlayerId(0))
        .expect("Festival controller passes");
    game.pass_priority(PlayerId(1)).expect("Festival resolves");
    assert_eq!(game.zone_of(drawn_card), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageShieldCreated { source, target, amount }
            if *source == festival && *target == Target::Player(PlayerId(0)) && *amount == 2
    )));

    game.pass_priority(PlayerId(0))
        .expect("priority passes to Char controller");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: char,
            targets: vec![Target::Player(PlayerId(0))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char casts after Festival");
    game.pass_priority(PlayerId(1))
        .expect("Char controller passes");
    game.pass_priority(PlayerId(0)).expect("Char resolves");
    assert_eq!(
        game.players[0].life, 18,
        "two of Char's four damage is prevented"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented { source, target, amount }
            if *source == char && *target == Target::Player(PlayerId(0)) && *amount == 2
    )));
    eprintln!("Festival trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Festival chosen-X shield preserves invariants");
}
