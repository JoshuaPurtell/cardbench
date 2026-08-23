//! Red discovery contract for Congregation at Dawn's ordered library search.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent,
    LibrarySearchDestination, ManaCost, PlayerId, Zone,
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

#[test]
#[allow(clippy::too_many_lines)] // One complete hidden-search/order trace is the contract.
fn congregation_at_dawn_privately_selects_reveals_shuffles_then_orders_up_to_three_creatures() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CONGREGATION-AT-DAWN")
        .expect("Congregation at Dawn definition exists");
    assert_eq!(definition.name, "Congregation at Dawn");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::Green, Color::Green, Color::White])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"private-up-to-three-creature-search-reveal-shuffle-ordered-library-top")
    );

    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let spell = game
        .add_card(PlayerId(0), "RAV-CONGREGATION-AT-DAWN", Zone::Hand)
        .expect("Congregation at Dawn is in hand");
    let watchwolf = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("first creature is in library");
    let grave_troll = game
        .add_card(PlayerId(0), "RAV-GOLGARI-GRAVE-TROLL", Zone::Library)
        .expect("second creature is in library");
    let stone_seeder = game
        .add_card(PlayerId(0), "RAV-STONE-SEEDER-HIEROPHANT", Zone::Library)
        .expect("third creature is in library");
    let noncreature = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Library)
        .expect("noncreature is in library");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("green payment exists");
    game.grant_mana(PlayerId(0), Color::White, 1)
        .expect("white payment exists");

    game.cast_spell(PlayerId(0), request(spell))
        .expect("Congregation at Dawn casts");
    game.pass_priority(PlayerId(0))
        .expect("controller passes Congregation at Dawn");
    game.pass_priority(PlayerId(1))
        .expect("Congregation at Dawn opens its private search decision");

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private creature search decision opens");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(decision.max_selections, 3);
    assert_eq!(decision.candidates.len(), 3);
    assert_eq!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .library_search_choice
            .expect("compatibility search view exists")
            .destination,
        LibrarySearchDestination::LibraryTop,
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .pending_decision
            .is_none(),
        "the opponent must not receive hidden library candidates"
    );
    assert!(
        !decision
            .candidates
            .iter()
            .any(|card| card.id == noncreature),
        "only creature cards are legal candidates"
    );

    game.submit_decision(
        PlayerId(0),
        decision.id,
        // This submitted order is the required new top-to-bottom order,
        // deliberately different from the library's original order.
        DecisionSelection::Objects(vec![grave_troll, watchwolf, stone_seeder]),
    )
    .expect("controller selects and orders three creature cards");

    assert_eq!(game.zone_of(grave_troll), Some(Zone::Library));
    assert_eq!(game.zone_of(watchwolf), Some(Zone::Library));
    assert_eq!(game.zone_of(stone_seeder), Some(Zone::Library));
    assert_eq!(game.zone_of(noncreature), Some(Zone::Library));
    assert_eq!(
        game.players[PlayerId(0).0]
            .library
            .iter()
            .rev()
            .take(3)
            .copied()
            .collect::<Vec<_>>(),
        vec![grave_troll, watchwolf, stone_seeder],
        "selected order becomes exact library top-to-bottom order after shuffling"
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
            GameEvent::LibraryShuffled { player: PlayerId(0), .. },
        ] if *source == spell
            && found == &vec![grave_troll, watchwolf, stone_seeder]
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardRevealed { player: PlayerId(0), card, .. }
            if *card == grave_troll || *card == watchwolf || *card == stone_seeder
    )));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::CardMoved { card, .. }
                if *card == grave_troll || *card == watchwolf || *card == stone_seeder
        )),
        "the searched cards remain library cards; ordering must not fabricate zone moves"
    );
    println!(
        "Congregation at Dawn ordered-search trace: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("ordered search preserves stack and library provenance invariants");

    let mut forged_placement = game.clone();
    let placement = forged_placement
        .event_log
        .iter_mut()
        .find_map(|event| match event {
            GameEvent::LibrarySearchTopCardsPlaced { top_to_bottom, .. } => Some(top_to_bottom),
            _ => None,
        })
        .expect("green trace records ordered placement provenance");
    placement.swap(0, 1);
    assert!(
        forged_placement.validate_invariants().is_err(),
        "an event-log placement order must agree with its selected search order"
    );
}
