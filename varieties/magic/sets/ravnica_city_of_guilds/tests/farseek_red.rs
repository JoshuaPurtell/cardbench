//! Red regression for Farseek's typed land search slice.

use cardbench_magic_engine::{CardType, CastRequest, Color, Game, GameEvent, PlayerId, Zone};
use cardbench_magic_rav::{card_definitions, rav_basic_land_type_bindings};

#[test]
fn farseek_has_its_exact_supported_casting_chassis() {
    let farseek = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FARSEEK")
        .expect("Farseek definition exists");
    assert_eq!(farseek.name, "Farseek");
    assert_eq!(farseek.mana_cost.generic, 1);
    assert_eq!(farseek.mana_cost.colored, vec![Color::Green]);
    assert_eq!(
        farseek.card_types,
        [CardType::Sorcery].into_iter().collect()
    );
    assert!(
        farseek
            .supported_rules
            .contains(&"library-nonforest-land-type-search")
    );
    assert!(
        farseek
            .supported_rules
            .contains(&"battlefield-tapped-land-entry")
    );
}

#[test]
fn farseek_returns_only_a_controller_owned_nonforest_typed_land_tapped() {
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV fixture builds with typed land lines");
    let farseek = game
        .add_card(PlayerId(0), "RAV-FARSEEK", Zone::Hand)
        .expect("Farseek setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("Forest setup");
    let plains = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("Plains setup");
    let opponents_island = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("opponent Island setup");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("Farseek mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: farseek,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Farseek casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert_eq!(game.zone_of(plains), Some(Zone::Battlefield));
    assert!(game.object(plains).expect("Plains persists").tapped);
    assert_eq!(game.zone_of(forest), Some(Zone::Library));
    assert_eq!(game.zone_of(opponents_island), Some(Zone::Library));
    assert_eq!(game.zone_of(farseek), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled {
            player: PlayerId(0),
            cards: 1,
        }
    )));
    println!("Farseek trace: {:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Farseek preserves zone, stack, and event invariants");
}
