//! Bounded public contract for Undercity Shade's black-only evasion.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn undercity_shade_definition_is_explicit_about_evasion_and_omitted_activation() {
    let shade = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-UNDERCITY-SHADE")
        .expect("Undercity Shade definition exists");
    assert_eq!(shade.name, "Undercity Shade");
    assert_eq!(shade.mana_cost, ManaCost::with_colors(4, [Color::Black]));
    assert_eq!(shade.colors, BTreeSet::from([Color::Black]));
    assert_eq!(shade.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((shade.power, shade.toughness), (Some(1), Some(1)));
    assert_eq!(shade.keywords, [Keyword::BlackEvasion]);
    assert!(shade.effects.is_empty());
    assert_eq!(
        shade.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "black-only-evasion"
        ]
    );
    assert!(!RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&shade.id));
}

#[test]
fn undercity_shade_public_scenario_rejects_a_nonblack_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_undercity_shade_black_evasion_compatibility")
        .expect("Undercity Shade public scenario exists");
    println!("Undercity Shade compatibility trace: {:?}", trace.event_log);
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
