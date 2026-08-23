//! Complete public RAV contract for Nightguard Patrol.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn nightguard_patrol_represents_its_complete_keyword_creature_behavior() {
    let patrol = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NIGHTGUARD-PATROL")
        .expect("Nightguard Patrol definition");
    assert_eq!(patrol.name, "Nightguard Patrol");
    assert_eq!(patrol.mana_cost, ManaCost::with_colors(2, [Color::White]));
    assert_eq!(patrol.colors, BTreeSet::from([Color::White]));
    assert_eq!(patrol.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(patrol.power, Some(2));
    assert_eq!(patrol.toughness, Some(1));
    assert_eq!(patrol.keywords, [Keyword::FirstStrike, Keyword::Vigilance]);
    assert!(patrol.effects.is_empty());
    assert_eq!(
        patrol.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "first-strike",
            "vigilance",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&patrol.id));
}

#[test]
fn nightguard_patrol_public_trace_covers_cast_vigilance_and_first_strike() {
    let scenarios = run_all_scenarios().expect("shown RAV scenarios run");
    let trace = scenarios
        .iter()
        .find(|scenario| scenario.id == "rav_nightguard_patrol_keywords")
        .expect("Nightguard Patrol public trace");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("SpellCast"))
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("FirstStrikeCombatDamage"))
    );
    assert!(trace.event_log.iter().any(|event| {
        event.contains("DamageDealtToPermanent")
            && event.contains("source: ObjectId(2)")
            && event.contains("permanent: ObjectId(3)")
    }));
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("source: ObjectId(3)")),
        "the normal blocker cannot assign damage after first strike"
    );
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("attacker: ObjectId(2), tapped: true")),
        "vigilance must preserve the attacker's untapped state"
    );
}
