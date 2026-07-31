//! Bounded public contract for Screeching Griffin's shared Flying behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn screeching_griffin_definition_is_explicit_about_the_omitted_activation() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SCREECHING-GRIFFIN")
        .expect("Screeching Griffin definition exists");

    assert_eq!(griffin.name, "Screeching Griffin");
    assert_eq!(griffin.mana_cost, ManaCost::with_colors(3, [Color::White]));
    assert_eq!(griffin.colors, BTreeSet::from([Color::White]));
    assert_eq!(griffin.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((griffin.power, griffin.toughness), (Some(2), Some(2)));
    assert_eq!(griffin.keywords, [Keyword::Flying]);
    assert!(griffin.effects.is_empty());
    assert_eq!(
        griffin.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&griffin.id),
        "the omitted activated ability keeps this definition bounded"
    );
}

#[test]
fn screeching_griffin_public_scenario_rejects_a_ground_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_screeching_griffin_flying_compatibility")
        .expect("Screeching Griffin public scenario exists");

    println!(
        "Screeching Griffin compatibility trace: {:?}",
        trace.event_log
    );
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
            .any(|event| event.contains("BlockersDeclared")),
        "an illegal ground block must not write a blocker declaration receipt"
    );
}
