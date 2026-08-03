//! Red regression: a counter-unless spell with a vanished target is countered
//! by the rules before it can open its resolution-time payment decision.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    Zone,
};

const TARGET: &str = "TST-STALE-COUNTER-UNLESS-TARGET";
const UNLESS: &str = "TST-STALE-COUNTER-UNLESS";
const COUNTER: &str = "TST-STALE-ORDINARY-COUNTER";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["stale-counter-unless-target-red"],
        power: None,
        toughness: None,
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
fn counter_unless_with_a_target_removed_by_another_counter_is_countered_by_rules() {
    let active = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new(
        [
            definition(TARGET, vec![Effect::GainLifeController { amount: 1 }]),
            definition(
                UNLESS,
                vec![Effect::CounterTargetSpellUnlessControllerPays {
                    mana_cost: ManaCost::new(1),
                }],
            ),
            definition(COUNTER, vec![Effect::CounterTargetSpell]),
        ],
        2,
    )
    .expect("fixture initializes");
    let target = game
        .add_card(active, TARGET, Zone::Hand)
        .expect("target spell begins in hand");
    let unless = game
        .add_card(responder, UNLESS, Zone::Hand)
        .expect("counter-unless begins in hand");
    let counter = game
        .add_card(responder, COUNTER, Zone::Hand)
        .expect("ordinary counter begins in hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        active,
        CastRequest {
            card: target,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("target spell casts");
    game.pass_priority(active)
        .expect("active player passes to responder");
    game.cast_spell(
        responder,
        CastRequest {
            card: unless,
            targets: vec![Target::Spell(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("counter-unless spell casts targeting the lower spell");
    game.cast_spell(
        responder,
        CastRequest {
            card: counter,
            targets: vec![Target::Spell(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("ordinary counter casts above the same target");

    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1, "only counter-unless remains");

    let first = game.priority;
    game.pass_priority(first)
        .expect("active player passes to the responder");
    let second = game.priority;
    let result = game.pass_priority(second);
    eprintln!(
        "stale counter-unless target result: {result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log(),
    );
    result.expect("a counter-unless spell with no remaining legal target is countered by rules");

    assert!(
        game.stack.is_empty(),
        "the stale counter-unless spell leaves stack"
    );
    assert_eq!(game.zone_of(unless), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCounteredByRules { card } if *card == unless
    )));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::DecisionOpened { .. })),
        "an all-illegal target must not expose a payment decision"
    );
    game.validate_invariants()
        .expect("stale-target countering preserves the stack state machine");
}
