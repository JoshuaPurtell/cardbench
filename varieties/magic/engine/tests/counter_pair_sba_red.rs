//! Red regression: CR 704.5q removes opposing +1/+1 and -1/-1 counter pairs
//! as a state-based action; derived P/T alone must not hide stale counters.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, CounterKind, Effect, Game, ManaCost, PlayerId, Target,
    Zone,
};

const CREATURE: &str = "COUNTER-PAIR-CREATURE";
const PLUS: &str = "COUNTER-PAIR-PLUS";
const MINUS: &str = "COUNTER-PAIR-MINUS";

fn definition(id: &'static str, card_type: &CardType, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type.clone()]),
        is_basic_land: false,
        supported_rules: &["counter-pair-SBA-probe"],
        power: (card_type == &CardType::Creature).then_some(2),
        toughness: (card_type == &CardType::Creature).then_some(2),
        keywords: vec![],
        effects,
    }
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player advances automatic step");
        game.pass_priority(PlayerId(1))
            .expect("opponent advances automatic step");
    }
}

fn cast_and_resolve(
    game: &mut Game,
    caster: PlayerId,
    opponent: PlayerId,
    spell: cardbench_magic_engine::ObjectId,
    target: cardbench_magic_engine::ObjectId,
) {
    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("counter spell casts");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(opponent).expect("spell resolves");
}

#[test]
fn state_based_actions_cancel_plus_and_minus_counter_pairs() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        [
            definition(CREATURE, &CardType::Creature, vec![]),
            definition(
                PLUS,
                &CardType::Instant,
                vec![Effect::AddCountersToTarget {
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 2,
                }],
            ),
            definition(
                MINUS,
                &CardType::Instant,
                vec![Effect::AddCountersToTarget {
                    counter: CounterKind::MinusOneMinusOne,
                    amount: 1,
                }],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let creature = game
        .put_on_battlefield(caster, CREATURE)
        .expect("creature setup");
    let plus = game
        .add_card(caster, PLUS, Zone::Hand)
        .expect("plus spell setup");
    let minus = game
        .add_card(caster, MINUS, Zone::Hand)
        .expect("minus spell setup");
    advance_to_precombat_main(&mut game);

    cast_and_resolve(&mut game, caster, opponent, plus, creature);
    cast_and_resolve(&mut game, caster, opponent, minus, creature);

    eprintln!("counter-pair SBA trace: {:?}", game.canonical_event_log());
    let counters = &game
        .object(creature)
        .expect("creature remains live")
        .counters;
    assert_eq!(
        counters.get(&CounterKind::PlusOnePlusOne),
        Some(&1),
        "one +1/+1 counter remains after one opposing pair cancels",
    );
    assert!(
        !counters.contains_key(&CounterKind::MinusOneMinusOne),
        "the -1/-1 counter must be removed as the same state-based action",
    );
    let expected_card = format!("card: {creature:?}");
    assert!(
        game.canonical_event_log().iter().any(|event| {
            event.contains("CounterPairsRemovedByStateBasedAction")
                && event.contains(&expected_card)
                && event.contains("amount: 1")
        }),
        "the source-free SBA cancellation must have an auditable receipt",
    );
    game.validate_invariants()
        .expect("counter-pair SBA state is auditable");
}
