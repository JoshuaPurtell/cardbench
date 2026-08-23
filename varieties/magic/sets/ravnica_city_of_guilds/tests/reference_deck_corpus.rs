//! Public corpus invariants for whole-deck Rust-policy development fixtures.

use std::collections::BTreeSet;

use cardbench_magic_rav::{
    CardSemanticStatus, RAV_CATALOG_COVERAGE_DECK_COUNT, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    card_definitions, load_catalog_coverage_decks, load_reference_decks, new_rav_game,
    rav_main_set_catalog,
};

#[test]
fn public_campaign_constructor_installs_an_invariant_valid_ruleset() {
    let game = new_rav_game(2).expect("fully bound two-player RAV game");
    game.validate_invariants()
        .expect("fully bound campaign setup satisfies every engine invariant");
}

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

#[test]
fn catalog_gauntlet_is_exact_sixty_and_covers_every_executable_identity_and_printing() {
    let decks = load_catalog_coverage_decks().expect("catalog coverage decks load");
    assert_eq!(decks.len(), RAV_CATALOG_COVERAGE_DECK_COUNT);
    assert_eq!(
        decks
            .iter()
            .map(|deck| deck.policy.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        6,
        "the all-card corpus rotates through six deterministic policy profiles"
    );
    let covered = decks
        .iter()
        .flat_map(|fixture| &fixture.deck.mainboard)
        .map(|entry| entry.card.as_str())
        .collect::<BTreeSet<_>>();
    let definitions = card_definitions()
        .into_iter()
        .map(|definition| definition.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        covered, definitions,
        "every executable identity is deck-covered"
    );

    for fixture in &decks {
        assert_eq!(
            fixture
                .deck
                .mainboard
                .iter()
                .map(|entry| u16::from(entry.count))
                .sum::<u16>(),
            60,
            "{} is exactly sixty cards",
            fixture.id
        );
        assert!(fixture.deck.sideboard.is_empty());
    }
    let covered_printings = rav_main_set_catalog()
        .into_iter()
        .filter(|card| match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
                covered.contains(definition_id)
            }
            CardSemanticStatus::CatalogOnly { .. } => false,
        })
        .count();
    assert_eq!(
        covered_printings, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "every catalog printing maps to a deck-covered executable identity"
    );
}
