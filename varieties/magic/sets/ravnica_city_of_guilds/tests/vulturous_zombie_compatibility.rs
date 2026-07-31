//! Bounded public contract for Vulturous Zombie's shared Flying rule.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn vulturous_zombie_definition_is_explicit_about_flying_and_omitted_trigger() {
    let zombie = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VULTUROUS-ZOMBIE")
        .expect("Vulturous Zombie definition exists");
    assert_eq!(zombie.name, "Vulturous Zombie");
    assert_eq!(
        zombie.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Green])
    );
    assert_eq!(zombie.colors, BTreeSet::from([Color::Black, Color::Green]));
    assert_eq!(zombie.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((zombie.power, zombie.toughness), (Some(3), Some(3)));
    assert_eq!(zombie.keywords, [Keyword::Flying]);
    assert!(zombie.effects.is_empty());
    assert_eq!(
        zombie.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
    assert!(!RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&zombie.id));
}

#[test]
fn vulturous_zombie_public_scenario_rejects_a_ground_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_vulturous_zombie_flying_compatibility")
        .expect("Vulturous Zombie public scenario exists");
    println!(
        "Vulturous Zombie compatibility trace: {:?}",
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
            .any(|event| event.contains("BlockersDeclared"))
    );
}
