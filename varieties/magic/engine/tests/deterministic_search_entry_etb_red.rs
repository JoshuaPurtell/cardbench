//! Red regression: deterministic library search must capture a selected
//! permanent's ETB while it is live, before post-resolution SBAs.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaCost, PlayerId, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const SEARCH: &str = "TST-DETERMINISTIC-SEARCH-ENTRY";
const EPHEMERAL: &str = "TST-DETERMINISTIC-SEARCH-EPHEMERAL";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let ephemeral = id == EPHEMERAL;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["deterministic-search-entry-etb-red"],
        power: ephemeral.then_some(0),
        toughness: ephemeral.then_some(0),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second pass resolves the deterministic search");
}

#[test]
fn deterministic_search_keeps_a_short_lived_entrants_etb_provenance() {
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
                    selection: LibrarySearchSelection::DeterministicFirstMatch,
                    reveal_selected: false,
                }],
            ),
            definition(EPHEMERAL, CardType::Creature, vec![]),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: EPHEMERAL,
            ability: TriggeredAbility {
                id: "ephemeral-etb",
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
        .add_card(controller, EPHEMERAL, Zone::Library)
        .expect("short-lived ETB creature begins in library");
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
    .expect("deterministic search casts");
    pass_pair(&mut game);

    eprintln!(
        "deterministic-search entry trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    let entry_move = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == creature))
        .expect("search moved the selected creature to the battlefield");
    let entry_incarnation = game.event_log[entry_move + 1..]
        .iter()
        .find_map(|event| match event {
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } if *object == creature => Some(*incarnation),
            _ => None,
        })
        .expect("entry has an incarnation receipt");
    let trigger_incarnation = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability,
                ..
            } if *source == creature && *ability == "ephemeral-etb" => Some(*source_incarnation),
            _ => None,
        })
        .expect("the short-lived search entrant must retain its ETB trigger");
    assert_eq!(trigger_incarnation, entry_incarnation);
    game.validate_invariants()
        .expect("deterministic search entry remains auditable");
}
