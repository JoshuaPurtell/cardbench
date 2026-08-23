//! Red regression: a permanent found to the battlefield during a private
//! library search must not place its ETB trigger before the search spell ends.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, LibrarySearchCardinality, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, PlayerId, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const SEARCH: &str = "TST-SEARCH-ENTRY-TRIGGER-ORDER";
const CREATURE: &str = "TST-SEARCH-ENTRY-TRIGGER-CREATURE";
const FRAGILE: &str = "TST-MULTI-SEARCH-FRAGILE-ETB";

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
        supported_rules: &["library-search-entry-trigger-order-red"],
        power: match id {
            FRAGILE => Some(0),
            _ if creature => Some(1),
            _ => None,
        },
        toughness: match id {
            FRAGILE => Some(0),
            _ if creature => Some(1),
            _ => None,
        },
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second pass opens the library search");
}

#[test]
fn searched_creature_etb_waits_until_after_its_search_spell_resolves() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                SEARCH,
                CardType::Instant,
                vec![Effect::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                        CardType::Creature,
                    ])),
                    destination: LibrarySearchDestination::Battlefield,
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: false,
                }],
            ),
            definition(CREATURE, CardType::Creature, vec![]),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: CREATURE,
            ability: TriggeredAbility {
                id: "creature-etb",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture constructs");
    let spell = game
        .add_card(controller, SEARCH, Zone::Hand)
        .expect("search spell begins in hand");
    let creature = game
        .add_card(controller, CREATURE, Zone::Library)
        .expect("ETB creature begins in library");
    game.begin_game().expect("game begins");
    game.cast_spell(
        controller,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("search spell casts");
    pass_pair(&mut game);
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("search opens a private controller decision");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);

    let result = game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![creature]),
    );
    eprintln!(
        "library-search entry-trigger result={result:?}; stack={:?}; pending={:?}; events={:?}",
        game.stack,
        game.view_for_player(controller)
            .expect("controller view after submitted decision")
            .pending_decision,
        game.canonical_event_log(),
    );
    result.expect("the legal search and creature entry must complete");

    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
    let spell_resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        .expect("search spell resolves before entry trigger placement");
    let etb_stacked = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == creature && *ability == "creature-etb"
            )
        })
        .expect("creature ETB stacks after the resolved spell lifecycle");
    assert!(spell_resolved < etb_stacked);
    game.validate_invariants()
        .expect("completed search entry keeps the stack state auditable");
}

#[test]
fn multi_search_captures_a_short_lived_entry_trigger_before_the_post_resolution_sba() {
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
                    cardinality: LibrarySearchCardinality::ZeroOrMore { maximum: 1 },
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: false,
                }],
            ),
            definition(FRAGILE, CardType::Creature, vec![]),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: FRAGILE,
            ability: TriggeredAbility {
                id: "fragile-etb",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture constructs");
    let spell = game
        .add_card(controller, SEARCH, Zone::Hand)
        .expect("search spell begins in hand");
    let creature = game
        .add_card(controller, FRAGILE, Zone::Library)
        .expect("zero-toughness ETB creature begins in library");
    game.begin_game().expect("game begins");
    game.cast_spell(
        controller,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("multi-search spell casts");
    pass_pair(&mut game);
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("multi-search opens a private controller decision");

    game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![creature]),
    )
    .expect("multi-search completes the selected short-lived entry");

    eprintln!(
        "multi-search short-lived entry trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    let spell_resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        .expect("multi-search spell resolves");
    let etb_stacked = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == creature && *ability == "fragile-etb"
            )
        })
        .expect("the historical battlefield ETB must still stack after SBA death");
    assert!(spell_resolved < etb_stacked);
    game.validate_invariants()
        .expect("short-lived multi-search entry keeps its trigger provenance auditable");
}
