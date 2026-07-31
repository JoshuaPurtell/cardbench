//! Full public contract for Moroii's Flying and upkeep life-loss rules.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn moroii_definition_is_explicit_about_flying_and_upkeep_trigger() {
    let moroii = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MOROII")
        .expect("Moroii definition exists");
    assert_eq!(moroii.name, "Moroii");
    assert_eq!(
        moroii.mana_cost,
        ManaCost::with_colors(2, [Color::Blue, Color::Black])
    );
    assert_eq!(moroii.colors, BTreeSet::from([Color::Blue, Color::Black]));
    assert_eq!(moroii.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((moroii.power, moroii.toughness), (Some(4), Some(4)));
    assert_eq!(moroii.keywords, [Keyword::Flying]);
    assert!(moroii.effects.is_empty());
    assert_eq!(
        moroii.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "upkeep-controller-life-loss",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&moroii.id));
}

#[test]
fn moroii_public_scenario_rejects_a_ground_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_moroii_flying_compatibility")
        .expect("Moroii public scenario exists");
    println!("Moroii compatibility trace: {:?}", trace.event_log);
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
