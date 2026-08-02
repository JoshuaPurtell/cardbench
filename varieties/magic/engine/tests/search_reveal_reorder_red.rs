//! Red regression for typed multi-card library search and public top-library
//! reveal/reorder decisions.
//!
//! The prior substrate only represented one selected card and could neither
//! preserve an ordered top-library selection nor distinguish a public reveal
//! from a private library search.  These contracts deliberately use synthetic
//! definitions so the core state machine is not coupled to a RAV card name.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection, Effect,
    Game, GameEvent, LibrarySearchCardinality, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaCost, PlayerId, Zone,
};

const MULTI_SEARCH: &str = "TST-MULTI-SEARCH";
const REORDER: &str = "TST-REVEAL-REORDER";
const CREATURE_A: &str = "TST-SEARCH-CREATURE-A";
const CREATURE_B: &str = "TST-SEARCH-CREATURE-B";
const CREATURE_C: &str = "TST-SEARCH-CREATURE-C";
const NONCREATURE: &str = "TST-SEARCH-NONCREATURE";

fn definition(id: &'static str, types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: types.clone(),
        is_basic_land: false,
        supported_rules: &["search-reveal-reorder-red"],
        power: types.contains(&CardType::Creature).then_some(1),
        toughness: types.contains(&CardType::Creature).then_some(1),
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new(
        [
            definition(
                MULTI_SEARCH,
                BTreeSet::from([CardType::Sorcery]),
                vec![Effect::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                        CardType::Creature,
                    ])),
                    destination: LibrarySearchDestination::Hand,
                    cardinality: LibrarySearchCardinality::ZeroOrMore { maximum: 2 },
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: true,
                    },
                    reveal_selected: false,
                }],
            ),
            definition(
                REORDER,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::RevealTopLibraryCardsAndReorder { count: 3 }],
            ),
            definition(CREATURE_A, BTreeSet::from([CardType::Creature]), vec![]),
            definition(CREATURE_B, BTreeSet::from([CardType::Creature]), vec![]),
            definition(CREATURE_C, BTreeSet::from([CardType::Creature]), vec![]),
            definition(NONCREATURE, BTreeSet::from([CardType::Artifact]), vec![]),
        ],
        2,
    )
    .expect("synthetic search fixture builds")
}

fn request(card: cardbench_magic_engine::ObjectId) -> CastRequest {
    CastRequest {
        card,
        targets: vec![],
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn private_multi_search_accepts_zero_or_more_typed_cards_and_shuffles_atomically() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), MULTI_SEARCH, Zone::Hand)
        .expect("search spell enters hand");
    let creature_a = game
        .add_card(PlayerId(0), CREATURE_A, Zone::Library)
        .expect("first creature enters library");
    let creature_b = game
        .add_card(PlayerId(0), CREATURE_B, Zone::Library)
        .expect("second creature enters library");
    let creature_c = game
        .add_card(PlayerId(0), CREATURE_C, Zone::Library)
        .expect("third creature enters library");
    let noncreature = game
        .add_card(PlayerId(0), NONCREATURE, Zone::Library)
        .expect("noncreature enters library");
    game.begin_game().expect("game begins");

    game.cast_spell(PlayerId(0), request(spell))
        .expect("search spell casts");
    pass_pair(&mut game);

    let controller = game.view_for_player(PlayerId(0)).expect("controller view");
    let decision = controller.pending_decision.expect("private search decision opens");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(decision.max_selections, 2);
    assert_eq!(decision.candidates.len(), 3);
    assert!(!decision.candidates.iter().any(|card| card.id == noncreature));
    assert!(game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .pending_decision
        .is_none(), "private candidates cannot project to the opponent");

    let before = game.canonical_event_log();
    assert!(game
        .submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Objects(vec![creature_a, creature_b, creature_c]),
        )
        .is_err(), "a zero-or-more maximum must reject an oversized selection");
    assert_eq!(game.canonical_event_log(), before, "rejected selection is atomic");

    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![creature_c, creature_a]),
    )
    .expect("two legal typed cards resolve the private search");

    assert_eq!(game.zone_of(creature_a), Some(Zone::Hand));
    assert_eq!(game.zone_of(creature_c), Some(Zone::Hand));
    assert_eq!(game.zone_of(creature_b), Some(Zone::Library));
    assert_eq!(game.zone_of(noncreature), Some(Zone::Library));
    assert!(!game.event_log.iter().any(|event| {
        matches!(event, GameEvent::CardRevealed { card, .. }
            if *card == creature_a || *card == creature_c)
    }), "private search must not silently reveal its selected cards");
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::LibrarySearchBatchResolved { player: PlayerId(0), source, found, destination: LibrarySearchDestination::Hand },
            GameEvent::LibraryShuffled { player: PlayerId(0), .. },
        ] if *source == spell && found == &vec![creature_c, creature_a]
    )), "the multi-card result and its shuffle must be adjacent receipts");
    game.validate_invariants()
        .expect("private multi-search preserves state-machine invariants");
}

#[test]
fn revealed_top_cards_are_public_and_policy_ordered_before_the_stack_continues() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), REORDER, Zone::Hand)
        .expect("reorder spell enters hand");
    let bottom = game
        .add_card(PlayerId(0), NONCREATURE, Zone::Library)
        .expect("bottom library card enters");
    let first_top = game
        .add_card(PlayerId(0), CREATURE_A, Zone::Library)
        .expect("first revealed card enters");
    let second_top = game
        .add_card(PlayerId(0), CREATURE_B, Zone::Library)
        .expect("second revealed card enters");
    let third_top = game
        .add_card(PlayerId(0), CREATURE_C, Zone::Library)
        .expect("top revealed card enters");
    game.begin_game().expect("game begins");

    game.cast_spell(PlayerId(0), request(spell))
        .expect("reorder spell casts");
    pass_pair(&mut game);

    let controller = game.view_for_player(PlayerId(0)).expect("controller view");
    let decision = controller.pending_decision.expect("reorder decision opens");
    assert_eq!(decision.kind, DecisionKind::LibraryReorder);
    assert_eq!(decision.min_selections, 3);
    assert_eq!(decision.max_selections, 3);
    assert_eq!(
        decision.candidates.iter().map(|card| card.id).collect::<Vec<_>>(),
        vec![third_top, second_top, first_top],
        "options are exposed top-to-bottom"
    );
    let opponent = game.view_for_player(PlayerId(1)).expect("opponent view");
    assert_eq!(opponent.pending_decision, Some(decision.clone()),
        "a revealed reorder must project its public option set to every player");
    assert_eq!(
        game.event_log.iter().filter(|event| matches!(event, GameEvent::CardRevealed { .. })).count(),
        3,
        "each looked-at card is revealed before the public decision opens"
    );

    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![first_top, third_top, second_top]),
    )
    .expect("submitted sequence defines new top-to-bottom order");
    assert_eq!(
        game.players[0].library,
        vec![bottom, second_top, third_top, first_top],
        "the last library slot is the top card after the ordered selection"
    );
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::LibraryReordered { player: PlayerId(0), top_to_bottom },
            GameEvent::SpellResolved { card },
        ] if top_to_bottom == &vec![first_top, third_top, second_top] && *card == spell
    )), "the ordering receipt must be complete before terminal spell resolution");
    game.validate_invariants()
        .expect("public reorder preserves state-machine invariants");
}
