//! Red regression for Transmute's required stack and hidden-search boundary.
//!
//! Transmute is an activated ability, not an immediate private-library move.
//! Paying its mana and discard costs must create a live stack object, give
//! every player a response window, and defer the controller-only library
//! selection until that object begins resolving.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Zone,
};

const TRANSMUTER: &str = "TST-STACK-TRANSMUTER";
const FOUND: &str = "TST-STACK-FOUND";

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: TRANSMUTER,
            name: "Stack Transmuter",
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["test-only"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![],
        },
        CardDefinition {
            id: FOUND,
            name: "Stack Found Card",
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["test-only"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

#[test]
fn transmute_costs_create_a_live_ability_before_the_private_search() {
    let player = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("fixture game initializes");
    let transmuter = game
        .add_card(player, TRANSMUTER, Zone::Hand)
        .expect("Transmute card starts in hand");
    let found = game
        .add_card(player, FOUND, Zone::Library)
        .expect("matching card starts hidden in the library");
    game.grant_mana(player, Color::Blue, 2)
        .expect("fixture provides colored Transmute cost");
    game.grant_mana(player, Color::White, 1)
        .expect("fixture provides generic Transmute cost");
    game.clear_event_log();

    // The old compatibility operation accepts a preselected hidden card and
    // completes immediately.  This assertion is intentionally red until the
    // activation is represented as a real stack ability.
    game.transmute(player, transmuter, Some(found))
        .expect("fixture can activate the legacy Transmute path");

    assert_eq!(game.stack.len(), 1, "Transmute must wait on the stack");
    assert_eq!(game.zone_of(transmuter), Some(Zone::Graveyard));
    assert_eq!(
        game.zone_of(found),
        Some(Zone::Library),
        "the selected library card cannot move before the response window closes"
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::AbilityActivated { source, ability, .. }
                if *source == transmuter && *ability == "transmute"
        )
    }));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::CardRevealed { .. })),
        "a hidden card is revealed only while Transmute resolves"
    );
}
