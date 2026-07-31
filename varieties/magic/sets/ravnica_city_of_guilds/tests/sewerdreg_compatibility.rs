//! Bounded public contract for Sewerdreg's static Fear behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn sewerdreg_definition_is_explicit_about_fear_and_omitted_regeneration() {
    let sewerdreg = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SEWERDREG")
        .expect("Sewerdreg definition exists");
    assert_eq!(sewerdreg.name, "Sewerdreg");
    assert_eq!(
        sewerdreg.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Black])
    );
    assert_eq!(sewerdreg.colors, BTreeSet::from([Color::Black]));
    assert_eq!(sewerdreg.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((sewerdreg.power, sewerdreg.toughness), (Some(3), Some(3)));
    assert_eq!(sewerdreg.keywords, [Keyword::Fear]);
    assert!(sewerdreg.effects.is_empty());
    assert_eq!(
        sewerdreg.supported_rules,
        ["colored-cost-casting", "base-characteristics", "fear"]
    );
    assert!(!RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sewerdreg.id));
}

#[test]
fn sewerdreg_public_scenario_rejects_a_nonblack_nonartifact_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_sewerdreg_fear_compatibility")
        .expect("Sewerdreg public scenario exists");
    println!("Sewerdreg compatibility trace: {:?}", trace.event_log);
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
        "an illegal Fear block must not write a blocker declaration receipt"
    );
}
