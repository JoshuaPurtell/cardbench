//! Red regression for private Aura searches with distinct printed names.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, LibrarySearchCardinality, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, PlayerId, TargetRequirement, Zone,
};

const SEARCH: &str = "TST-DISTINCT-AURA-SEARCH";
const AURA_A: &str = "TST-DISTINCT-AURA-A";
const AURA_B: &str = "TST-DISTINCT-AURA-B";
const AURA_C: &str = "TST-DISTINCT-AURA-C";
const ENCHANTMENT: &str = "TST-DISTINCT-AURA-NON-AURA";

fn definition(
    id: &'static str,
    name: &'static str,
    card_type: CardType,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["distinct-aura-library-search-contract"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn aura(id: &'static str, name: &'static str) -> CardDefinition {
    definition(
        id,
        name,
        CardType::Enchantment,
        vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![],
        }],
    )
}

fn request(card: cardbench_magic_engine::ObjectId) -> CastRequest {
    CastRequest {
        card,
        targets: vec![],
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One selection trace proves private filtering and unique-name atomicity.
fn private_aura_search_filters_non_auras_and_rejects_same_printed_name() {
    let mut game = Game::new(
        vec![
            definition(
                SEARCH,
                "Search",
                CardType::Sorcery,
                vec![Effect::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::Aura,
                    destination: LibrarySearchDestination::Hand,
                    cardinality: LibrarySearchCardinality::ZeroOrMoreDistinctNames { maximum: 3 },
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: true,
                }],
            ),
            aura(AURA_A, "Shared Aura"),
            aura(AURA_B, "Shared Aura"),
            aura(AURA_C, "Other Aura"),
            definition(ENCHANTMENT, "Not An Aura", CardType::Enchantment, vec![]),
        ],
        2,
    )
    .expect("synthetic search fixture builds");
    let search = game
        .add_card(PlayerId(0), SEARCH, Zone::Hand)
        .expect("search spell setup");
    let first_shared = game
        .add_card(PlayerId(0), AURA_A, Zone::Library)
        .expect("first same-name Aura setup");
    let second_shared = game
        .add_card(PlayerId(0), AURA_B, Zone::Library)
        .expect("second same-name Aura setup");
    let other = game
        .add_card(PlayerId(0), AURA_C, Zone::Library)
        .expect("different-name Aura setup");
    let non_aura = game
        .add_card(PlayerId(0), ENCHANTMENT, Zone::Library)
        .expect("non-Aura Enchantment setup");

    game.cast_spell(PlayerId(0), request(search))
        .expect("search spell casts");
    game.pass_priority(PlayerId(0))
        .expect("search controller passes");
    game.pass_priority(PlayerId(1))
        .expect("search resolves to a private decision");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private decision opens");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(decision.max_selections, 3);
    assert_eq!(decision.candidates.len(), 3);
    assert!(!decision.candidates.iter().any(|candidate| candidate.id == non_aura));
    assert!(game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .pending_decision
        .is_none());

    let events_before = game.canonical_event_log();
    assert!(game
        .submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Objects(vec![first_shared, second_shared]),
        )
        .is_err());
    assert_eq!(game.canonical_event_log(), events_before);
    assert_eq!(game.zone_of(first_shared), Some(Zone::Library));
    assert_eq!(game.zone_of(second_shared), Some(Zone::Library));

    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![first_shared, other]),
    )
    .expect("differently named Auras resolve the search");
    assert_eq!(game.zone_of(first_shared), Some(Zone::Hand));
    assert_eq!(game.zone_of(other), Some(Zone::Hand));
    assert_eq!(game.zone_of(second_shared), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchBatchResolved { source, found, .. }
            if *source == search && found == &vec![first_shared, other]
    )));
    game.validate_invariants()
        .expect("distinct Aura search lifecycle is replay-valid");
}
