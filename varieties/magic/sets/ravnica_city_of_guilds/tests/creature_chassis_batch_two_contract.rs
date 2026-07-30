//! Public contract for the second bounded RAV creature-chassis batch.
//!
//! These compatibility definitions deliberately expose normal casting and base
//! characteristics only. No printed activated, triggered, evasion, token, or
//! combat behavior is represented by this test or the executable slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
fn second_creature_chassis_batch_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();
    let expected = [
        (
            "RAV-ELVISH-SKYSWEEPER",
            "Elvish Skysweeper",
            ManaCost::with_colors(0, [Color::Green]),
            BTreeSet::from([Color::Green]),
            1,
            1,
        ),
        (
            "RAV-FRENZIED-GOBLIN",
            "Frenzied Goblin",
            ManaCost::with_colors(0, [Color::Red]),
            BTreeSet::from([Color::Red]),
            1,
            1,
        ),
        (
            "RAV-GRAYSCALED-GHARIAL",
            "Grayscaled Gharial",
            ManaCost::with_colors(0, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            1,
            1,
        ),
        (
            "RAV-GREATER-FORGELING",
            "Greater Forgeling",
            ManaCost::with_colors(3, [Color::Red, Color::Red]),
            BTreeSet::from([Color::Red]),
            3,
            4,
        ),
        (
            "RAV-GOLIATH-SPIDER",
            "Goliath Spider",
            ManaCost::with_colors(6, [Color::Green, Color::Green]),
            BTreeSet::from([Color::Green]),
            7,
            6,
        ),
        (
            "RAV-IVY-DANCER",
            "Ivy Dancer",
            ManaCost::with_colors(2, [Color::Green]),
            BTreeSet::from([Color::Green]),
            1,
            2,
        ),
        (
            "RAV-LORE-BROKER",
            "Lore Broker",
            ManaCost::with_colors(1, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            1,
            2,
        ),
        (
            "RAV-MORTIPEDE",
            "Mortipede",
            ManaCost::with_colors(3, [Color::Black]),
            BTreeSet::from([Color::Black]),
            4,
            1,
        ),
        (
            "RAV-SELESNYA-EVANGEL",
            "Selesnya Evangel",
            ManaCost::with_colors(0, [Color::Green, Color::White]),
            BTreeSet::from([Color::Green, Color::White]),
            1,
            2,
        ),
        (
            "RAV-SELESNYA-SAGITTARS",
            "Selesnya Sagittars",
            ManaCost::with_colors(3, [Color::Green, Color::White]),
            BTreeSet::from([Color::Green, Color::White]),
            2,
            5,
        ),
    ];

    for (id, name, mana_cost, colors, power, toughness) in expected {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .unwrap_or_else(|| panic!("missing public RAV definition {id}"));
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(definition.mana_cost, mana_cost, "{id}");
        assert_eq!(definition.colors, colors, "{id}");
        assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
        assert_eq!(definition.power, Some(power), "{id}");
        assert_eq!(definition.toughness, Some(toughness), "{id}");
        assert_eq!(
            definition.supported_rules,
            ["colored-cost-casting", "base-characteristics"],
            "{id} must not present an unsupported card-specific ability"
        );
        assert!(definition.keywords.is_empty(), "{id}");
        assert!(definition.effects.is_empty(), "{id}");
    }
}

#[test]
fn public_second_chassis_scenarios_cover_costs_stack_zones_and_base_pt() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_one_mana_creature_chassis",
        "rav_red_black_creature_chassis_two",
        "rav_forgeling_dancer_creature_chassis",
        "rav_spider_sagittars_creature_chassis",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
