//! Red regression: one policy-submitted library search may occur at an exact
//! instruction inside a multi-effect stack spell.
//!
//! The existing search continuation only accepts a one-effect spell, which
//! blocks otherwise ordinary prefix/search/suffix card definitions before
//! costs or stack placement. The eventual continuation must keep the spell
//! live, make the search private, and resume its suffix exactly once.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, LibrarySearchDestination, LibrarySearchRequirement, LibrarySearchSelection,
    ManaCost, PlayerId, Zone,
};

const SPELL: &str = "TST-MULTI-INSTRUCTION-LIBRARY-SEARCH";
const CREATURE: &str = "TST-MULTI-INSTRUCTION-LIBRARY-CREATURE";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["multi-instruction-library-search-red"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
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
#[allow(clippy::too_many_lines)] // The full private-search stack transcript is the regression contract.
fn a_middle_policy_submitted_library_search_suspends_and_resumes_its_spell() {
    let controller = PlayerId(0);
    let mut game = Game::new(
        [
            definition(
                SPELL,
                CardType::Instant,
                vec![
                    Effect::GainLifeController { amount: 1 },
                    Effect::SearchControllerLibrary {
                        requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                            CardType::Creature,
                        ])),
                        destination: LibrarySearchDestination::Hand,
                        selection: LibrarySearchSelection::PolicySubmitted {
                            may_fail_to_find: false,
                        },
                        reveal_selected: false,
                    },
                    Effect::GainLifeController { amount: 2 },
                ],
            ),
            definition(CREATURE, CardType::Creature, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let spell = game
        .add_card(controller, SPELL, Zone::Hand)
        .expect("spell begins in hand");
    let selected = game
        .add_card(controller, CREATURE, Zone::Library)
        .expect("matching creature begins in library");
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
    .expect("a multi-instruction policy search spell casts");
    pass_pair(&mut game);

    eprintln!(
        "multi-instruction library search red trace: stack={:?}; pending={:?}; events={:?}",
        game.stack,
        game.view_for_player(controller)
            .expect("controller view")
            .pending_decision,
        game.canonical_event_log(),
    );
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("the middle library search must open a private decision");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.candidates.len(), 1);
    assert_eq!(game.stack.len(), 1, "the resolving spell remains live");
    assert_eq!(game.player(controller).expect("controller exists").life, 21);
    game.validate_invariants()
        .expect("paused private search remains state-machine valid");

    game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![selected]),
    )
    .expect("controller selects the matching library card");
    assert_eq!(game.zone_of(selected), Some(Zone::Hand));
    assert_eq!(game.player(controller).expect("controller exists").life, 23);
    assert!(game.stack.is_empty(), "search suffix resolves exactly once");
    let events = &game.event_log;
    let prefix = events
        .iter()
        .position(|event| {
            matches!(event, GameEvent::LifeGained { player, amount } if *player == controller && *amount == 1)
        })
        .expect("prefix life receipt");
    let found = events
        .iter()
        .position(|event| {
            matches!(event, GameEvent::LibrarySearchResolved { source, found: Some(card), .. } if *source == spell && *card == selected)
        })
        .expect("selected-card search receipt");
    let suffix = events
        .iter()
        .position(|event| {
            matches!(event, GameEvent::LifeGained { player, amount } if *player == controller && *amount == 2)
        })
        .expect("suffix life receipt");
    let resolved = events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        .expect("one terminal spell receipt");
    assert!(prefix < found && found < suffix && suffix < resolved);
    eprintln!(
        "multi-instruction library search green trace: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("completed private search remains state-machine valid");
}
