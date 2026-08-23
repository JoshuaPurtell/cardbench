//! Red regression: later targeted instructions must revalidate their target.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, ManaCost, PlayerId, Target, Zone,
};

const LOWER_SPELL: &str = "STACK-LATER-REVALIDATION-LOWER";
const DOUBLE_COUNTER: &str = "STACK-LATER-REVALIDATION-DOUBLE-COUNTER";

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
        supported_rules: &["stack-later-target-revalidation-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn second_targeted_instruction_skips_a_spell_target_removed_by_the_first() {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new(
        [
            instant(LOWER_SPELL, vec![Effect::GainLifeController { amount: 1 }]),
            instant(
                DOUBLE_COUNTER,
                vec![
                    Effect::CounterTargetInstantOrSorcerySpell,
                    Effect::CounterTargetInstantOrSorcerySpell,
                ],
            ),
        ],
        2,
    )
    .expect("fixture game initializes");
    let lower = game
        .add_card(caster, LOWER_SPELL, Zone::Hand)
        .expect("lower spell enters hand");
    let response = game
        .add_card(responder, DOUBLE_COUNTER, Zone::Hand)
        .expect("response enters hand");

    game.cast_spell(
        caster,
        CastRequest {
            card: lower,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("lower spell is cast");
    game.pass_priority(caster)
        .expect("caster opens the response window");
    game.cast_spell(
        responder,
        CastRequest {
            card: response,
            targets: vec![Target::Spell(lower), Target::Spell(lower)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("response selects the lower spell for both independent occurrences");
    game.pass_priority(responder)
        .expect("responder passes after casting");

    let before_stack = game.stack.clone();
    let before_events = game.canonical_event_log();
    let result = game.pass_priority(caster);
    eprintln!(
        "double-counter resolution result: {result:?}; before_stack: {before_stack:?}; after_stack: {:?}; before_events: {before_events:?}; after_events: {:?}",
        game.stack,
        game.canonical_event_log(),
    );

    assert!(
        result.is_ok(),
        "after the first counter removes the lower spell, the later targeted instruction must be skipped rather than reject and roll back the complete resolution"
    );
    assert!(
        game.stack.is_empty(),
        "both resolved spell objects leave the stack"
    );
    assert_eq!(game.zone_of(lower), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(response), Some(Zone::Graveyard));
    let events = game.canonical_event_log();
    assert_eq!(
        &events[before_events.len()..],
        [
            format!("PriorityPassed {{ player: {caster:?} }}"),
            format!("SpellCountered {{ card: {lower:?}, source: {response:?} }}"),
            format!("CardMoved {{ card: {lower:?}, to: Graveyard }}"),
            format!("ObjectIncarnationAdvanced {{ object: {lower:?}, incarnation: 3 }}"),
            format!(
                "TargetInstructionSkipped {{ card: {response:?}, effect_index: 1, target: Spell({lower:?}) }}"
            ),
            format!("SpellResolved {{ card: {response:?} }}"),
            format!("CardMoved {{ card: {response:?}, to: Graveyard }}"),
            format!("ObjectIncarnationAdvanced {{ object: {response:?}, incarnation: 3 }}"),
        ],
        "the first counter, later target skip, and response lifecycle have one canonical order"
    );
    game.validate_invariants()
        .expect("a completed partial targeted resolution preserves invariants");
}
