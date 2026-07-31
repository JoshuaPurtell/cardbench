//! Exact public contract for the RAV basic-land compatibility boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

const BASIC_LANDS: [(&str, &str, Color); 5] = [
    ("RAV-PLAINS", "Plains", Color::White),
    ("RAV-ISLAND", "Island", Color::Blue),
    ("RAV-SWAMP", "Swamp", Color::Black),
    ("RAV-MOUNTAIN", "Mountain", Color::Red),
    ("RAV-FOREST", "Forest", Color::Green),
];

#[test]
fn rav_basic_lands_are_exact_single_color_mana_compatibility_slices() {
    let definitions = card_definitions();
    for (id, name, color) in BASIC_LANDS {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .expect("every RAV basic land definition exists");
        assert_eq!(definition.name, name);
        assert_eq!(definition.mana_cost, ManaCost::new(0));
        assert!(definition.colors.is_empty());
        assert_eq!(definition.mana_colors, BTreeSet::from([color]));
        assert_eq!(definition.card_types, BTreeSet::from([CardType::Land]));
        assert!(definition.is_basic_land);
        assert_eq!(
            definition.supported_rules,
            [
                "basic-land-deck-construction",
                "intrinsic-single-color-mana-ability",
            ]
        );
        assert!(definition.keywords.is_empty());
        assert!(definition.effects.is_empty());
        assert!(
            !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id),
            "{name} remains bounded until mana abilities can be activated while paying a cost"
        );
    }
}

#[test]
fn basic_land_public_trace_records_each_supported_nonstack_mana_result() {
    let scenario = run_all_scenarios()
        .expect("public RAV scenarios run")
        .into_iter()
        .find(|result| result.id == "rav_basic_land_intrinsic_mana_slice")
        .expect("basic-land public scenario exists");

    assert_eq!(scenario.digest, "fnv1a64:2ebe6d7e14855cd8");
    assert_eq!(scenario.event_log.len(), BASIC_LANDS.len() * 2);
    assert_eq!(
        scenario
            .event_log
            .iter()
            .filter(|event| event.contains("ManaAbilityActivated"))
            .count(),
        BASIC_LANDS.len()
    );
    assert_eq!(
        scenario
            .event_log
            .iter()
            .filter(|event| event.contains("ManaAdded"))
            .count(),
        BASIC_LANDS.len()
    );
    for (_, _, color) in BASIC_LANDS {
        assert!(scenario.event_log.iter().any(|event| {
            event.contains(&format!(
                "ManaAdded {{ player: PlayerId(0), color: {color:?}, amount: 1 }}"
            ))
        }));
    }
    for receipt_pair in scenario.event_log.chunks_exact(2) {
        assert!(receipt_pair[0].contains("ManaAbilityActivated"));
        assert!(receipt_pair[1].contains("ManaAdded"));
    }
    assert!(scenario.event_log.iter().all(|event| {
        !event.contains("SpellCast")
            && !event.contains("SpellResolved")
            && !event.contains("PriorityPassed")
    }));
}
