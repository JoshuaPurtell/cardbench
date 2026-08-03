//! Red regression: target legality is sampled when a spell starts resolving,
//! not again after an in-resolution private decision resumes its suffix.
//!
//! The spell deliberately targets the same creature twice.  Its first
//! instruction exiles that creature, then a policy-submitted library search
//! pauses resolution before the second targeted instruction.  CR 608.2b does
//! not counter a spell that already began resolving merely because its first
//! instruction made the remaining repeated target unavailable.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionSelection, Effect, Game, GameEvent,
    LibrarySearchDestination, LibrarySearchRequirement, LibrarySearchSelection, ManaCost,
    PlayerId, Target, Zone,
};

const SPELL: &str = "TST-RESUMED-TARGET-SNAPSHOT";
const TARGET: &str = "TST-RESUMED-TARGET";
const SEARCH_CARD: &str = "TST-RESUMED-SEARCH-CREATURE";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["resumed-resolution-target-snapshot-red"],
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
    game.pass_priority(second)
        .expect("second player starts resolution");
}

#[test]
fn private_search_resume_keeps_the_initial_target_legality_snapshot() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                SPELL,
                CardType::Instant,
                vec![
                    Effect::ExileTargetCreature,
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
                    Effect::ExileTargetCreature,
                ],
            ),
            definition(TARGET, CardType::Creature, vec![]),
            definition(SEARCH_CARD, CardType::Creature, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let spell = game
        .add_card(controller, SPELL, Zone::Hand)
        .expect("spell begins in hand");
    let target = game
        .put_on_battlefield(opponent, TARGET)
        .expect("repeated target begins on battlefield");
    let selected = game
        .add_card(controller, SEARCH_CARD, Zone::Library)
        .expect("search candidate begins in library");
    game.begin_game().expect("game begins");

    game.cast_spell(
        controller,
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target), Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("repeated-target spell casts");
    pass_pair(&mut game);

    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("middle search opens a private decision");
    assert_eq!(game.zone_of(target), Some(Zone::Exile));
    assert_eq!(game.stack.len(), 1, "spell stays live while search is private");

    let result = game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![selected]),
    );
    eprintln!(
        "resumed target snapshot result={result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log(),
    );
    result.expect("the already-resolving spell must finish after its private search");

    assert_eq!(game.zone_of(selected), Some(Zone::Hand));
    assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::TargetInstructionSkipped { card, effect_index: 2, target: Target::Permanent(id) } if *card == spell && *id == target)
    }));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::SpellResolved { card } if *card == spell)
    }));
    game.validate_invariants()
        .expect("resumed stack resolution remains state-machine valid");
}
