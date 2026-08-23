//! Direct ability-complete contracts for the small RAV Flying/Reach slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn audited_rav_flying_and_reach_creatures_are_complete_and_positive_manifest_entries() {
    let definitions = card_definitions();
    let expected = [
        (
            "RAV-CONCLAVE-EQUENAUT",
            ManaCost::with_colors(4, [Color::White, Color::White]),
            BTreeSet::from([Color::White]),
            (3, 3),
            vec![Keyword::Convoke, Keyword::Flying],
            vec![
                "full-rules-fidelity",
                "convoke",
                "base-characteristics",
                "flying",
            ],
        ),
        (
            "RAV-SNAPPING-DRAKE",
            ManaCost::with_colors(3, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            (3, 2),
            vec![Keyword::Flying],
            vec![
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
            ],
        ),
        (
            "RAV-GOLIATH-SPIDER",
            ManaCost::with_colors(6, [Color::Green, Color::Green]),
            BTreeSet::from([Color::Green]),
            (7, 6),
            vec![Keyword::Reach],
            vec![
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "reach",
            ],
        ),
        (
            "RAV-COURIER-HAWK",
            ManaCost::with_colors(1, [Color::White]),
            BTreeSet::from([Color::White]),
            (1, 2),
            vec![Keyword::Flying, Keyword::Vigilance],
            vec![
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "vigilance",
            ],
        ),
    ];

    for (id, mana_cost, colors, power_toughness, keywords, supported_rules) in expected {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .unwrap_or_else(|| panic!("missing RAV definition {id}"));
        assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id));
        assert_eq!(definition.mana_cost, mana_cost, "{id}");
        assert_eq!(definition.colors, colors, "{id}");
        assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
        assert_eq!(
            (definition.power, definition.toughness),
            (Some(power_toughness.0), Some(power_toughness.1)),
            "{id}"
        );
        assert_eq!(definition.keywords, keywords, "{id}");
        assert_eq!(definition.supported_rules, supported_rules, "{id}");
        assert!(definition.effects.is_empty(), "{id}");
    }
}

#[test]
fn public_traces_exercise_flying_reach_and_vigilance_declaration_receipts() {
    let scenarios = run_all_scenarios().expect("public RAV scenarios run");
    for (id, markers) in [
        (
            "rav_conclave_equenaut_flying_goliath_spider_reach_block",
            ["AttackersDeclared", "BlockersDeclared"].as_slice(),
        ),
        (
            "rav_snapping_drake_flying_courier_hawk_flying_block",
            ["AttackersDeclared", "BlockersDeclared"].as_slice(),
        ),
        (
            "rav_courier_hawk_flying_vigilance_attack",
            ["AttackersDeclared"].as_slice(),
        ),
    ] {
        let scenario = scenarios
            .iter()
            .find(|scenario| scenario.id == id)
            .unwrap_or_else(|| panic!("missing public scenario {id}"));
        for marker in markers {
            assert!(
                scenario
                    .event_log
                    .iter()
                    .any(|event| event.contains(marker)),
                "{id} lacks {marker}: {:?}",
                scenario.event_log
            );
        }
    }
}
