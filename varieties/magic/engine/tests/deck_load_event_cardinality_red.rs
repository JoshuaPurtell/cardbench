//! Red regression: deck-load receipts must never saturate their card count.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, DeckEntry, DeckList, Game, ManaCost, PlayerId, RulesError,
};

const CARD: &str = "TST-DECK-CARDINALITY";

fn definition() -> CardDefinition {
    CardDefinition {
        id: CARD,
        name: CARD,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["test-only-deck-cardinality"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn deck_larger_than_event_cardinality_is_rejected_before_loading() {
    let max_receiptable_cards = usize::from(u16::MAX);
    let deck = DeckList {
        // No constructed-format policy applies to this engine primitive, so
        // expansion setup may supply an arbitrary public list. Its receipt
        // field is u16 and must not silently saturate at 65,535.
        mainboard: (0..258)
            .map(|_| DeckEntry {
                card: CARD.to_owned(),
                count: u8::MAX,
            })
            .collect(),
        sideboard: vec![],
    };
    assert!(
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            > max_receiptable_cards
    );

    let mut game = Game::new([definition()], 2).expect("fixture initializes");
    let result = game.load_deck_into_library(PlayerId(0), &deck);
    eprintln!(
        "oversized deck load: {result:?}; library_len={}; events={:?}",
        game.player(PlayerId(0))
            .expect("player exists")
            .library
            .len(),
        game.canonical_event_log(),
    );
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "deck card count exceeds event receipt range"
        ))
    ));
    assert!(
        game.player(PlayerId(0))
            .expect("player exists")
            .library
            .is_empty(),
        "a rejected oversized deck must not retain a partial library"
    );
    assert!(
        game.canonical_event_log().is_empty(),
        "a rejected oversized deck must not retain setup receipts"
    );
    game.validate_invariants()
        .expect("a rejected setup batch leaves a valid empty fixture");
}
