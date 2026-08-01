//! Regression for Vigor Mortis's spent-green graveyard return.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, ManaCost, ManaPaymentSelection, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn vigor_mortis_has_its_exact_targeted_graveyard_return_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VIGOR-MORTIS")
        .expect("Vigor Mortis definition exists");
    assert_eq!(definition.name, "Vigor Mortis");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Black])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Sorcery].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"return-target-creature-card-from-graveyard-to-battlefield")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"spent-green-plus-one-plus-one-counter")
    );
}

fn resolve_with_generic_spend(generic: [Color; 2]) -> (Game, cardbench_magic_engine::ObjectId) {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    let spell = game
        .add_card(PlayerId(0), "RAV-VIGOR-MORTIS", Zone::Hand)
        .expect("spell setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("creature target setup");
    game.grant_mana(PlayerId(0), Color::Black, 2)
        .expect("black symbols");
    for color in generic {
        game.grant_mana(PlayerId(0), color, 1)
            .expect("generic payment mana");
    }

    game.cast_spell_with_mana_spend(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: generic.to_vec(),
            hybrid: vec![],
        },
    )
    .expect("Vigor Mortis casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    (game, target)
}

#[test]
fn vigor_mortis_returns_its_owners_creature_and_uses_spent_green_for_counter() {
    let (game, target) = resolve_with_generic_spend([Color::Green, Color::Green]);
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(
        game.characteristics(target)
            .expect("returned creature characteristics")
            .power,
        Some(4)
    );
    assert_eq!(
        game.characteristics(target)
            .expect("returned creature characteristics")
            .toughness,
        Some(4)
    );
    assert_eq!(
        game.object(target)
            .expect("returned creature remains an object")
            .counters
            .get("+1/+1"),
        Some(&1)
    );
    println!("Vigor Mortis trace: {:?}", game.canonical_event_log());
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            cardbench_magic_engine::GameEvent::CounterPlaced {
                card,
                counter: "+1/+1",
                amount: 1,
                ..
            } if *card == target
        )
    }));
    game.validate_invariants()
        .expect("spent-green counter return stays valid");
}

#[test]
fn vigor_mortis_without_spent_green_returns_without_a_counter() {
    let (game, target) = resolve_with_generic_spend([Color::Black, Color::Black]);
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(
        game.characteristics(target)
            .expect("returned creature characteristics")
            .power,
        Some(3)
    );
    game.validate_invariants()
        .expect("non-green return stays valid");
}

#[test]
#[allow(clippy::too_many_lines)] // The two independent return lifecycles share one auditable state trace.
fn vigor_mortis_counter_clears_on_death_before_a_later_reentry() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    let first_vigor = game
        .add_card(PlayerId(0), "RAV-VIGOR-MORTIS", Zone::Hand)
        .expect("first Vigor setup");
    let second_vigor = game
        .add_card(PlayerId(0), "RAV-VIGOR-MORTIS", Zone::Hand)
        .expect("second Vigor setup");
    let first_gasp = game
        .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
        .expect("first Last Gasp setup");
    let second_gasp = game
        .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
        .expect("second Last Gasp setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("creature target setup");
    game.grant_mana(PlayerId(0), Color::Black, 8)
        .expect("black payment supply");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("green first-cast supply");

    game.cast_spell_with_mana_spend(
        PlayerId(0),
        CastRequest {
            card: first_vigor,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![Color::Green, Color::Green],
            hybrid: vec![],
        },
    )
    .expect("first Vigor casts");
    game.pass_priority(PlayerId(0)).expect("first Vigor pass");
    game.pass_priority(PlayerId(1))
        .expect("first Vigor resolves");
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(
        game.object(target)
            .expect("target exists")
            .counters
            .get("+1/+1"),
        Some(&1)
    );

    for gasp in [first_gasp, second_gasp] {
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: gasp,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect("Last Gasp casts");
        game.pass_priority(PlayerId(0))
            .expect("caster passes Last Gasp");
        game.pass_priority(PlayerId(1))
            .expect("opponent resolves Last Gasp");
    }
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert!(
        game.object(target)
            .expect("target persists as a graveyard card")
            .counters
            .is_empty(),
        "battlefield counter cannot survive a zone change"
    );

    game.cast_spell_with_mana_spend(
        PlayerId(0),
        CastRequest {
            card: second_vigor,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![Color::Black, Color::Black],
            hybrid: vec![],
        },
    )
    .expect("second Vigor casts");
    game.pass_priority(PlayerId(0)).expect("second Vigor pass");
    game.pass_priority(PlayerId(1))
        .expect("second Vigor resolves");
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(
        game.characteristics(target)
            .expect("later returned creature characteristics")
            .power,
        Some(3),
        "later reentry cannot retain the first Vigor counter"
    );
    assert!(
        game.object(target)
            .expect("later returned creature exists")
            .counters
            .is_empty()
    );
    game.validate_invariants()
        .expect("counter lifecycle remains valid through zone changes");
}
