//! Bounded public contract for Tattered Drake's shared Flying behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn tattered_drake_definition_is_explicit_about_the_omitted_regeneration() {
    let drake = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TATTERED-DRAKE")
        .expect("Tattered Drake definition exists");
    assert_eq!(drake.mana_cost, ManaCost::with_colors(4, [Color::Blue]));
    assert_eq!(drake.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(drake.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((drake.power, drake.toughness), (Some(2), Some(2)));
    assert_eq!(drake.keywords, [Keyword::Flying]);
    assert!(drake.effects.is_empty());
    assert_eq!(
        drake.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
    assert!(!RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&drake.id));
}

#[test]
fn tattered_drake_public_scenario_rejects_a_ground_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_tattered_drake_flying_compatibility")
        .expect("Tattered Drake public scenario exists");
    println!("Tattered Drake compatibility trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:1dcdb2289fdaa917");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("AttackersDeclared"))
    );
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("BlockersDeclared"))
    );
}
