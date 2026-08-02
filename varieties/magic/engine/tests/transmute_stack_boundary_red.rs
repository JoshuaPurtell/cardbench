//! Red regression for Transmute's required stack and hidden-search boundary.
//!
//! Transmute is an activated ability, not an immediate private-library move.
//! Paying its mana and discard costs must create a live stack object, give
//! every player a response window, and defer the controller-only library
//! selection until that object begins resolving.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionSelection, Game, GameEvent, Keyword, ManaCost,
    PlayerId, Zone,
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

    game.activate_transmute(player, transmuter)
        .expect("Transmute costs create a stack ability");

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

    game.pass_priority(player)
        .expect("activating player passes on the stack ability");
    game.pass_priority(PlayerId(1))
        .expect("opponent pass starts Transmute resolution");
    let controller_view = game
        .view_for_player(player)
        .expect("controller receives the private decision view");
    let decision = controller_view
        .pending_decision
        .expect("Transmute opens an id-bearing private search decision");
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        [found]
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent receives a safe projection")
            .pending_decision
            .is_none(),
        "the opponent cannot see Transmute's hidden-library candidates"
    );
    assert!(
        game.pass_priority(player).is_err(),
        "the private search decision blocks ordinary priority actions"
    );
    game.submit_decision(player, decision.id, DecisionSelection::Objects(vec![found]))
        .expect("controller selects the matching card while Transmute resolves");
    assert_eq!(game.zone_of(found), Some(Zone::Hand));
    assert_eq!(game.stack.len(), 0);
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::Transmuted { discarded, found: Some(selected), .. }
                if *discarded == transmuter && *selected == found
        )
    }));
    game.validate_invariants()
        .expect("stack-backed Transmute retains a complete event lifecycle");
}
