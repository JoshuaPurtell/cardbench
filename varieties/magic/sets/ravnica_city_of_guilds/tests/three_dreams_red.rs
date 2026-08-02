//! Red discovery contract for Three Dreams' distinct-name Aura search.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, ManaCost,
    PlayerId, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn request(card: cardbench_magic_engine::ObjectId) -> CastRequest {
    CastRequest {
        card,
        targets: vec![],
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_to_search_decision(game: &mut Game, spell: cardbench_magic_engine::ObjectId) {
    game.cast_spell(PlayerId(0), request(spell))
        .expect("Three Dreams casts");
    game.pass_priority(PlayerId(0))
        .expect("controller passes Three Dreams");
    game.pass_priority(PlayerId(1))
        .expect("Three Dreams opens its private search decision");
}

#[test]
fn three_dreams_has_its_exact_private_distinct_aura_search_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-THREE-DREAMS")
        .expect("Three Dreams definition exists");
    assert_eq!(definition.name, "Three Dreams");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Sorcery]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition
        .supported_rules
        .contains(&"private-up-to-three-distinct-name-aura-search-reveal-hand-shuffle"));
}

#[test]
#[allow(clippy::too_many_lines)] // One causal hidden-zone trace proves the selection boundary.
fn three_dreams_reveals_up_to_three_differently_named_auras_then_shuffles() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let spell = game
        .add_card(PlayerId(0), "RAV-THREE-DREAMS", Zone::Hand)
        .expect("Three Dreams is in hand");
    let cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Library)
        .expect("first Aura is in library");
    let darkness = game
        .add_card(PlayerId(0), "RAV-CLINGING-DARKNESS", Zone::Library)
        .expect("second Aura is in library");
    let flickerform = game
        .add_card(PlayerId(0), "RAV-FLICKERFORM", Zone::Library)
        .expect("third Aura is in library");
    let fetters = game
        .add_card(PlayerId(0), "RAV-FAITHS-FETTERS", Zone::Library)
        .expect("fourth Aura is in library");
    let non_aura = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("non-Aura is in library");
    game.grant_mana(PlayerId(0), Color::White, 5)
        .expect("Three Dreams payment exists");
    resolve_to_search_decision(&mut game, spell);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private Aura search decision opens");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(decision.max_selections, 3);
    assert_eq!(decision.candidates.len(), 4);
    assert!(game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .pending_decision
        .is_none());
    assert!(!decision.candidates.iter().any(|card| card.id == non_aura));

    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![cloak, darkness, flickerform]),
    )
    .expect("three differently named Auras are a legal answer");
    assert_eq!(game.zone_of(cloak), Some(Zone::Hand));
    assert_eq!(game.zone_of(darkness), Some(Zone::Hand));
    assert_eq!(game.zone_of(flickerform), Some(Zone::Hand));
    assert_eq!(game.zone_of(fetters), Some(Zone::Library));
    assert_eq!(game.zone_of(non_aura), Some(Zone::Library));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardRevealed { .. }))
            .count(),
        3
    );
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::LibrarySearchBatchResolved {
                player: PlayerId(0),
                source,
                found,
                ..
            },
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            }
        ] if *source == spell && found == &vec![cloak, darkness, flickerform]
    )));
    eprintln!("Three Dreams trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Three Dreams search preserves invariant state");
}

#[test]
fn three_dreams_rejects_same_name_selection_without_mutating_the_pending_search() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let spell = game
        .add_card(PlayerId(0), "RAV-THREE-DREAMS", Zone::Hand)
        .expect("Three Dreams is in hand");
    let first_cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Library)
        .expect("first same-name Aura is in library");
    let second_cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Library)
        .expect("second same-name Aura is in library");
    let flickerform = game
        .add_card(PlayerId(0), "RAV-FLICKERFORM", Zone::Library)
        .expect("differently named Aura is in library");
    game.grant_mana(PlayerId(0), Color::White, 5)
        .expect("Three Dreams payment exists");
    resolve_to_search_decision(&mut game, spell);
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private Aura search decision opens");
    let events_before = game.canonical_event_log();
    assert!(game
        .submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Objects(vec![first_cloak, second_cloak]),
        )
        .is_err());
    assert_eq!(game.canonical_event_log(), events_before);
    assert_eq!(game.zone_of(first_cloak), Some(Zone::Library));
    assert_eq!(game.zone_of(second_cloak), Some(Zone::Library));
    assert_eq!(game.zone_of(flickerform), Some(Zone::Library));
    assert!(game
        .view_for_player(PlayerId(0))
        .expect("controller view after rejected answer")
        .pending_decision
        .is_some());
    game.validate_invariants()
        .expect("rejected same-name answer is atomic");
}
