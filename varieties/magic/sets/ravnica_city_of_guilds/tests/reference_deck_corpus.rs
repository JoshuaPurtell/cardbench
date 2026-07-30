//! Public corpus invariants for whole-deck Rust-policy development fixtures.

use std::collections::BTreeSet;

use cardbench_magic_rav::load_reference_decks;

#[test]
fn public_policy_corpus_is_distinct_and_exactly_sixty_cards_per_deck() {
    let decks = load_reference_decks().expect("shown RAV policy corpus loads");
    assert!(
        decks.len() >= 12,
        "the engine-finding corpus must retain a broad public deck population"
    );

    let mut deck_ids = BTreeSet::new();
    let mut policy_ids = BTreeSet::new();
    for fixture in decks {
        assert!(
            deck_ids.insert(fixture.id.clone()),
            "duplicate shown deck id `{}`",
            fixture.id
        );
        assert!(
            policy_ids.insert(fixture.policy.clone()),
            "each public deck needs its own reproducible Rust policy; duplicate `{}`",
            fixture.policy
        );
        let mainboard_cards: u16 = fixture
            .deck
            .mainboard
            .iter()
            .map(|entry| u16::from(entry.count))
            .sum();
        assert_eq!(
            mainboard_cards, 60,
            "fixture `{}` must be an exact sixty-card engine probe",
            fixture.id
        );
        assert!(
            fixture.deck.sideboard.is_empty(),
            "fixture `{}` has no sideboard substrate yet",
            fixture.id
        );
    }
}
