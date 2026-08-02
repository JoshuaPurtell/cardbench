//! Red regression: declining an optional trigger must skip a deferred search.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionKind, Effect, Game, GameEvent,
    LibrarySearchCardinality, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, PlayerId, PolicyAction, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const SOURCE: &str = "TST-OPTIONAL-SEARCH-SOURCE";
const MATCH: &str = "TST-OPTIONAL-SEARCH-MATCH";
const ABILITY: &str = "optional-entry-search";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["optional-trigger-decline-search-red"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn declined_optional_trigger_skips_its_private_library_search_instead_of_opening_a_decision() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        [creature(SOURCE), creature(MATCH)],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: SOURCE,
            ability: TriggeredAbility {
                id: ABILITY,
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::ManaValueExactly(0),
                    destination: LibrarySearchDestination::Hand,
                    cardinality: LibrarySearchCardinality::ZeroOrMore { maximum: u8::MAX },
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: true,
                }],
            },
        }],
    )
    .expect("trigger fixture builds");
    let source = game
        .add_card(PlayerId(0), SOURCE, Zone::Hand)
        .expect("source setup");
    let matching = game
        .add_card(PlayerId(0), MATCH, Zone::Library)
        .expect("matching card setup");

    game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: source,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("source casts");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1))
        .expect("source resolves and trigger stacks");
    game.pass_priority(PlayerId(0))
        .expect("controller passes trigger");
    game.pass_priority(PlayerId(1))
        .expect("opponent opens optional decision");
    game.submit_policy_move(
        PlayerId(0),
        "test.optional-trigger-decline-search.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            source,
            ability: ABILITY,
            pay: false,
            target: None,
        },
    )
    .expect("controller declines optional trigger");

    let view = game.view_for_player(PlayerId(0)).expect("controller view");
    println!(
        "optional declined-search trace: pending={:?}; events={:#?}",
        view.pending_decision,
        game.canonical_event_log()
    );
    assert!(
        view.pending_decision.is_none(),
        "declining must resolve the trigger directly rather than opening its deferred library search"
    );
    assert_eq!(game.zone_of(matching), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source: resolved, ability, .. }
            if *resolved == source && *ability == ABILITY
    )));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DecisionOpened {
                kind: DecisionKind::LibrarySearch,
                ..
            }
        )),
        "a declined optional trigger must not expose private library contents"
    );
    game.validate_invariants()
        .expect("declining remains an invariant-valid terminal transition");
}
