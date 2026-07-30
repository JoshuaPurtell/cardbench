//! Complete public RAV contract for Boros Recruit.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, HybridManaSymbol, Keyword, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
fn boros_recruit_exposes_full_supported_hybrid_and_first_strike_behavior() {
    let recruit = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BOROS-RECRUIT")
        .expect("Boros Recruit definition");
    assert_eq!(recruit.name, "Boros Recruit");
    assert_eq!(
        recruit.mana_cost,
        ManaCost::with_hybrid(
            0,
            [],
            [HybridManaSymbol {
                first: Color::Red,
                second: Color::White,
            }],
        )
    );
    assert_eq!(recruit.colors, BTreeSet::from([Color::Red, Color::White]));
    assert_eq!(recruit.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(recruit.power, Some(1));
    assert_eq!(recruit.toughness, Some(1));
    assert_eq!(recruit.keywords, vec![Keyword::FirstStrike]);
    assert_eq!(recruit.effects, Vec::new());
    assert_eq!(
        recruit.supported_rules,
        ["hybrid-cost-casting", "first-strike"],
        "the complete card has no omitted printed behavior"
    );
}

#[test]
fn boros_recruit_public_trace_covers_both_payment_choices_and_first_strike_damage() {
    let scenarios = run_all_scenarios().expect("shown RAV scenarios run");
    let trace = scenarios
        .iter()
        .find(|scenario| scenario.id == "rav_boros_recruit_hybrid_first_strike")
        .expect("Boros Recruit public trace");
    assert_eq!(
        trace
            .event_log
            .iter()
            .filter(|event| event.contains("SpellCast"))
            .count(),
        2,
        "one Recruit is cast through each legal hybrid payment path"
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("step: FirstStrikeCombatDamage"))
    );
    assert!(trace.event_log.iter().any(|event| {
        event.contains("DamageDealtToPermanent")
            && event.contains("source: ObjectId(4)")
            && event.contains("permanent: ObjectId(3)")
    }));
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("source: ObjectId(3)")),
        "the lethal normal blocker cannot assign damage after first strike"
    );
}
