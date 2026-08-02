//! Red regression: a stack-spell target must have existed before its source.

#[path = "support/stack_fixture.rs"]
mod stack_fixture;

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, ManaCost, PlayerId, RulesError, StackObject,
    StackObjectId, Target, Zone,
};

const LOWER_SPELL: &str = "STACK-TARGET-ORDER-LOWER";
const COUNTER_SPELL: &str = "STACK-TARGET-ORDER-COUNTER";
const WARMUP: &str = "STACK-TARGET-ORDER-WARMUP";

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
        supported_rules: &["stack-target-order-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn invariant_audit_rejects_a_stack_spell_targeting_itself() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new(
        [
            instant(LOWER_SPELL, vec![Effect::GainLifeController { amount: 1 }]),
            instant(
                COUNTER_SPELL,
                vec![Effect::CounterTargetInstantOrSorcerySpell],
            ),
            instant(WARMUP, vec![Effect::GainLifeController { amount: 1 }]),
        ],
        2,
    )
    .expect("fixture game initializes");
    stack_fixture::advance_stack_identity(&mut game, WARMUP);
    stack_fixture::advance_stack_identity(&mut game, WARMUP);
    let lower = game
        .add_card(second, LOWER_SPELL, Zone::Hand)
        .expect("lower spell enters hand");
    let counter = game
        .add_card(first, COUNTER_SPELL, Zone::Hand)
        .expect("counter spell enters hand");
    game.players[first.0].hand.clear();
    game.players[second.0].hand.clear();
    game.stack = vec![
        StackObject {
            id: StackObjectId(1),
            card: lower,
            source_incarnation: 1,
            source_colors: BTreeSet::new(),
            controller: second,
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
            card: counter,
            source_incarnation: 1,
            source_colors: BTreeSet::new(),
            controller: first,
            ability_id: None,
            targets: vec![Target::Spell(counter)],
            target_incarnations: vec![],
            effects: vec![Effect::CounterTargetInstantOrSorcerySpell],
            chosen_x: None,
            chosen_color: None,
            mana_spent: None,
            convoke_symbols: 0,
            generic_cost_reduction: 0,
        },
    ];

    let audit = game.validate_invariants();
    eprintln!(
        "self-target stack audit result: {audit:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log(),
    );
    assert!(matches!(
        audit,
        Err(RulesError::IllegalAction(
            "a stack spell target must be lower than its source"
        ))
    ));
}
