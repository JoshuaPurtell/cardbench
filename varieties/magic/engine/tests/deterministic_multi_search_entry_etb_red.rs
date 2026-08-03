//! Red regression: deterministic multi-search battlefield entry keeps ETB LKI.
//!
//! The deterministic multi-search resolver is a separate compatibility path
//! from policy-submitted multi-search. A short-lived selected permanent must
//! still capture its ETB before the source spell's post-resolution SBA check.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, LibrarySearchCardinality,
    LibrarySearchDestination, LibrarySearchRequirement, LibrarySearchSelection, ManaCost, PlayerId,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const SEARCH: &str = "TST-DETERMINISTIC-MULTI-SEARCH-ETB";
const FRAGILE: &str = "TST-DETERMINISTIC-MULTI-SEARCH-FRAGILE";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let fragile = id == FRAGILE;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["deterministic-multi-search-entry-etb-red"],
        power: fragile.then_some(0),
        toughness: fragile.then_some(0),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

#[test]
fn deterministic_multi_search_retains_a_short_lived_selected_creatures_etb() {
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
                    selection: LibrarySearchSelection::DeterministicFirstMatch,
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
    .expect("fixture initializes");
    let spell = game
        .add_card(controller, SEARCH, Zone::Hand)
        .expect("search spell setup");
    let creature = game
        .add_card(controller, FRAGILE, Zone::Library)
        .expect("fragile creature setup");
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
    eprintln!(
        "deterministic multi-search short-lived entry trace={:?}",
        game.canonical_event_log()
    );

    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    let entry_incarnation = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } if *object == creature && *incarnation > 1 => Some(*incarnation),
            _ => None,
        })
        .expect("selected creature enters with a fresh incarnation");
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability: "fragile-etb",
                ..
            } if *source == creature && *source_incarnation == entry_incarnation
        )),
        "deterministic multi-search must retain the selected creature's historical ETB"
    );
    game.validate_invariants()
        .expect("deterministic multi-search entry keeps a valid state");
}
