//! Red regression: a stack-spell target must have existed before its source.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, ManaCost, PlayerId, StackObject, Target, Zone,
};

const LOWER_SPELL: &str = "STACK-TARGET-ORDER-LOWER";
const COUNTER_SPELL: &str = "STACK-TARGET-ORDER-COUNTER";

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
        ],
        2,
    )
    .expect("fixture game initializes");
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
            card: lower,
            controller: second,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 1 }],
        },
        StackObject {
            card: counter,
            controller: first,
            targets: vec![Target::Spell(counter)],
            effects: vec![Effect::CounterTargetInstantOrSorcerySpell],
        },
    ];

    let audit = game.validate_invariants();
    eprintln!(
        "self-target stack audit result: {audit:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log(),
    );
    assert!(
        audit.is_err(),
        "a spell cannot target itself because it was not on the stack before its cast"
    );
}
