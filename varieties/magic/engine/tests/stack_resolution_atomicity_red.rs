//! Red regression: a failed top-stack resolution must roll back its final pass.

#[path = "support/stack_fixture.rs"]
mod stack_fixture;

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, ManaCost, PlayerId, RulesError, StackObject,
    StackObjectId, Target, TargetRequirement, Zone,
};

const TARGET_SPELL: &str = "STACK-ATOMICITY-TARGET";
const MALFORMED_SPELL: &str = "STACK-ATOMICITY-MALFORMED";
const WARMUP: &str = "STACK-ATOMICITY-WARMUP";

fn instant(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["stack-resolution-atomicity-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn failed_resolution_restores_the_final_pass_and_authoritative_stack() {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new(
        [
            instant(TARGET_SPELL, vec![Effect::GainLifeController { amount: 1 }]),
            // The public fixture seam can represent this malformed but
            // shape-valid effect. Its target remains a legal instant spell
            // through resolution, so the resolver reaches the unsupported
            // `DealDamage`/`Target::Spell` dispatch and returns an error.
            instant(
                MALFORMED_SPELL,
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::InstantOrSorcerySpell,
                }],
            ),
            instant(WARMUP, vec![Effect::GainLifeController { amount: 1 }]),
        ],
        2,
    )
    .expect("fixture game initializes");
    stack_fixture::advance_stack_identity(&mut game, WARMUP);
    stack_fixture::advance_stack_identity(&mut game, WARMUP);
    let target = game
        .add_card(responder, TARGET_SPELL, Zone::Hand)
        .expect("target spell enters hand");
    let malformed = game
        .add_card(caster, MALFORMED_SPELL, Zone::Hand)
        .expect("malformed spell enters hand");
    game.players[responder.0].hand.clear();
    game.players[caster.0].hand.clear();
    game.stack = vec![
        StackObject {
            id: StackObjectId(1),
            card: target,
            source_incarnation: 1,
            source_colors: BTreeSet::new(),
            controller: responder,
            ability_id: None,
            targets: vec![],
            target_incarnations: vec![],
            effects: vec![Effect::GainLifeController { amount: 1 }],
            chosen_x: None,
            chosen_color: None,
            mana_spent: None,
            convoke_symbols: 0,
            generic_cost_reduction: 0,
        },
        StackObject {
            id: StackObjectId(2),
            card: malformed,
            source_incarnation: 1,
            source_colors: BTreeSet::new(),
            controller: caster,
            ability_id: None,
            targets: vec![Target::Spell(target)],
            target_incarnations: vec![],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::InstantOrSorcerySpell,
            }],
            chosen_x: None,
            chosen_color: None,
            mana_spent: None,
            convoke_symbols: 0,
            generic_cost_reduction: 0,
        },
    ];
    game.clear_event_log();

    game.pass_priority(caster)
        .expect("first pass opens the final response window");
    let before_stack = game.stack.clone();
    let before_events = game.canonical_event_log();
    let before_priority = game.priority;
    let before_turn = game.turn;
    let before_step = game.step;

    let result = game.pass_priority(responder);
    eprintln!(
        "failed-resolution result: {result:?}; before_stack: {before_stack:?}; after_stack: {:?}; before_events: {before_events:?}; after_events: {:?}; before_priority: {before_priority:?}; after_priority: {:?}",
        game.stack,
        game.canonical_event_log(),
        game.priority,
    );

    assert!(
        matches!(result, Err(RulesError::IllegalTarget(Target::Spell(card))) if card == target),
        "the malformed resolution must reject its stack-spell damage target; result={result:?}"
    );
    assert_eq!(
        game.stack, before_stack,
        "a rejected final pass popped or rewrote the authoritative stack"
    );
    assert_eq!(
        game.canonical_event_log(),
        before_events,
        "a rejected final pass left an auditable receipt behind"
    );
    assert_eq!(game.priority, before_priority);
    assert_eq!(game.turn, before_turn);
    assert_eq!(game.step, before_step);
}
