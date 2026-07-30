//! Red-phase regression probes for turn-lifecycle defects found by audit.
//!
//! These tests intentionally specify the Rules-correct outcome before a fix is
//! applied.  They are not a substitute for the repository's green contracts.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, Keyword, ManaCost, PlayerId, Zone,
};

const TRANSMUTER: &str = "AUDIT-TRANSMUTER";
const FOUND: &str = "AUDIT-FOUND";

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: TRANSMUTER,
            name: "Audit Transmuter",
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["transmute"],
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
            name: "Audit Found Card",
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["fixture"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

#[test]
fn transmute_activation_keeps_priority_with_its_controller() {
    let player = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("fixture game initializes");
    let transmuter = game
        .add_card(player, TRANSMUTER, Zone::Hand)
        .expect("transmute card is in hand");
    let found = game
        .add_card(player, FOUND, Zone::Library)
        .expect("matching library card exists");
    game.grant_mana(player, Color::Blue, 2)
        .expect("fixture provides transmute mana");
    game.grant_mana(player, Color::White, 1)
        .expect("fixture provides generic transmute mana");

    game.transmute(player, transmuter, found)
        .expect("a legal activated ability resolves in the supported slice");
    eprintln!("transmute event log: {:?}", game.event_log);

    // CR 117.3c: the player who had priority retains it after activating an
    // ability. Transmute is an activated ability even though this substrate
    // models its supported resolution atomically.
    assert_eq!(game.priority, player);
    game.validate_invariants()
        .expect("the retained-priority state must remain internally valid");
}
