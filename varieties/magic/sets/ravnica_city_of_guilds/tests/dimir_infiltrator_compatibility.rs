//! Public compatibility and full-fidelity contract for Dimir Infiltrator.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn dimir_infiltrator_represents_complete_static_evasion_and_stack_transmute_rules() {
    let infiltrator = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-INFILTRATOR")
        .expect("Dimir Infiltrator definition exists");
    assert_eq!(infiltrator.name, "Dimir Infiltrator");
    assert_eq!(
        infiltrator.mana_cost,
        ManaCost::with_colors(0, [Color::Blue, Color::Black])
    );
    assert_eq!(
        infiltrator.colors,
        BTreeSet::from([Color::Blue, Color::Black])
    );
    assert_eq!(infiltrator.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(
        (infiltrator.power, infiltrator.toughness),
        (Some(1), Some(3))
    );
    assert_eq!(
        infiltrator.keywords,
        [
            Keyword::Unblockable,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Black])),
        ]
    );
    assert!(infiltrator.effects.is_empty());
    assert_eq!(
        infiltrator.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "unblockable",
            "stack-backed-private-transmute",
        ]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&infiltrator.id),
        "static unblockability and stack-backed Transmute are fully represented"
    );
}

#[test]
fn public_scenario_preserves_unblockable_declaration_provenance() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_dimir_infiltrator_unblockable_transmute_compatibility")
        .expect("Dimir Infiltrator public scenario exists");
    println!(
        "Dimir Infiltrator compatibility trace: {:?}",
        trace.event_log
    );
    assert_eq!(
        trace.digest, "fnv1a64:9366feebbf6d7203",
        "the Rust contract must track the checked-in public scenario digest"
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("Transmuted"))
    );
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
        "an illegal unblockable block must not write a blocker declaration receipt"
    );
}
