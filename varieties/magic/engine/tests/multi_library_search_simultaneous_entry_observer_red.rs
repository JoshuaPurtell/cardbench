//! Red regression: a multi-card battlefield search is one simultaneous entry.
//!
//! The second selected card has an entry observer. CR 603.6a requires that
//! observer to see both selected creatures, including the first card which
//! was selected earlier in library order.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, LibrarySearchCardinality, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, PlayerId, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const SEARCH: &str = "TST-MULTI-SEARCH-SIMULTANEOUS-ENTRY";
const BODY: &str = "TST-MULTI-SEARCH-SIMULTANEOUS-BODY";
const WATCHER: &str = "TST-MULTI-SEARCH-SIMULTANEOUS-WATCHER";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["multi-library-search-simultaneous-entry-observer-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn multi_card_library_search_uses_one_entry_observer_snapshot() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                SEARCH,
                CardType::Instant,
                vec![Effect::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                        CardType::Creature,
                    ])),
                    destination: LibrarySearchDestination::Battlefield,
                    cardinality: LibrarySearchCardinality::Exactly(2),
                    selection: LibrarySearchSelection::DeterministicFirstMatch,
                    reveal_selected: false,
                }],
            ),
            definition(BODY, CardType::Creature, vec![]),
            definition(WATCHER, CardType::Creature, vec![]),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: WATCHER,
            ability: TriggeredAbility {
                id: "observe-controlled-nonartifact-entry",
                condition: TriggerCondition::ControlledNonartifactPermanentEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::ReturnAnotherControlledPermanentSharingEnteredCardTypes],
            },
        }],
    )
    .expect("fixture initializes");
    let search = game
        .add_card(controller, SEARCH, Zone::Hand)
        .expect("search spell setup");
    // The deterministic compatibility selector retains the library's public
    // fixture order. The body therefore enters first and the observer second.
    let body = game
        .add_card(controller, BODY, Zone::Library)
        .expect("first selected creature setup");
    let watcher = game
        .add_card(controller, WATCHER, Zone::Library)
        .expect("second selected creature setup");
    game.begin_game().expect("game begins");
    game.cast_spell(
        controller,
        CastRequest {
            card: search,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("two-card search casts");
    pass_pair(&mut game);

    let order = game
        .view_for_player(controller)
        .expect("controller view is available")
        .pending_decision
        .expect("two simultaneous observer events require an explicit order");
    assert_eq!(order.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(order.trigger_candidates.len(), 2);
    assert!(order.trigger_candidates.iter().all(|entry| {
        entry.source == watcher && entry.ability == "observe-controlled-nonartifact-entry"
    }));
    game.submit_decision(
        controller,
        order.id,
        DecisionSelection::TriggerOrder(order.trigger_candidates.clone()),
    )
    .expect("controller orders both simultaneous entry observations");

    let observer_triggers = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked {
                    source,
                    ability: "observe-controlled-nonartifact-entry",
                    ..
                } if *source == watcher
            )
        })
        .count();
    eprintln!(
        "multi-library-search simultaneous-entry trace={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(body), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(watcher), Some(Zone::Battlefield));
    assert_eq!(
        observer_triggers, 2,
        "the second selected observer must see itself and the first simultaneous entrant"
    );
    game.validate_invariants()
        .expect("multi-search entry state remains invariant-valid");
}

#[test]
#[allow(clippy::too_many_lines)] // Both public selection and resulting trigger-order boundaries are the regression.
fn policy_selected_multi_search_uses_the_same_entry_observer_snapshot() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                SEARCH,
                CardType::Instant,
                vec![Effect::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                        CardType::Creature,
                    ])),
                    destination: LibrarySearchDestination::Battlefield,
                    cardinality: LibrarySearchCardinality::Exactly(2),
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: false,
                }],
            ),
            definition(BODY, CardType::Creature, vec![]),
            definition(WATCHER, CardType::Creature, vec![]),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: WATCHER,
            ability: TriggeredAbility {
                id: "observe-controlled-nonartifact-entry",
                condition: TriggerCondition::ControlledNonartifactPermanentEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::ReturnAnotherControlledPermanentSharingEnteredCardTypes],
            },
        }],
    )
    .expect("fixture initializes");
    let search = game
        .add_card(controller, SEARCH, Zone::Hand)
        .expect("search spell setup");
    let body = game
        .add_card(controller, BODY, Zone::Library)
        .expect("first selected creature setup");
    let watcher = game
        .add_card(controller, WATCHER, Zone::Library)
        .expect("second selected creature setup");
    game.begin_game().expect("game begins");
    game.cast_spell(
        controller,
        CastRequest {
            card: search,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("two-card policy search casts");
    pass_pair(&mut game);
    let search_decision = game
        .view_for_player(controller)
        .expect("controller view is available")
        .pending_decision
        .expect("private multi-search decision opens");
    assert_eq!(search_decision.kind, DecisionKind::LibrarySearch);
    game.submit_decision(
        controller,
        search_decision.id,
        DecisionSelection::Objects(vec![body, watcher]),
    )
    .expect("controller selects both creatures");

    let order = game
        .view_for_player(controller)
        .expect("controller view is available")
        .pending_decision
        .expect("two simultaneous observer events require an explicit order");
    assert_eq!(order.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(order.trigger_candidates.len(), 2);
    game.submit_decision(
        controller,
        order.id,
        DecisionSelection::TriggerOrder(order.trigger_candidates.clone()),
    )
    .expect("controller orders both simultaneous entry observations");

    let observer_triggers = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked {
                    source,
                    ability: "observe-controlled-nonartifact-entry",
                    ..
                } if *source == watcher
            )
        })
        .count();
    eprintln!(
        "policy multi-library-search simultaneous-entry trace={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(body), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(watcher), Some(Zone::Battlefield));
    assert_eq!(observer_triggers, 2);
    game.validate_invariants()
        .expect("policy multi-search entry state remains invariant-valid");
}
