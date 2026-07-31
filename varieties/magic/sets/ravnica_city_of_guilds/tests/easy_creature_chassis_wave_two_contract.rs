//! Public contract for a low-complexity RAV creature-chassis wave.
//!
//! Each compatibility definition deliberately supports only normal casting and
//! base characteristics. Printed keywords, activated abilities, and triggered
//! abilities stay outside the executable slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Declarative public fact table stays together for auditability.
fn easy_creature_wave_two_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();
    let expected = [
        (
            "RAV-BENEVOLENT-ANCESTOR",
            "Benevolent Ancestor",
            ManaCost::with_colors(2, [Color::White]),
            BTreeSet::from([Color::White]),
            0,
            4,
        ),
        (
            "RAV-SURVEILLING-SPRITE",
            "Surveilling Sprite",
            ManaCost::with_colors(1, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            1,
            1,
        ),
        (
            "RAV-TERRAFORMER",
            "Terraformer",
            ManaCost::with_colors(2, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            2,
            2,
        ),
        (
            "RAV-ROOFSTALKER-WIGHT",
            "Roofstalker Wight",
            ManaCost::with_colors(1, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            2,
            1,
        ),
        (
            "RAV-SEWERDREG",
            "Sewerdreg",
            ManaCost::with_colors(3, [Color::Black, Color::Black]),
            BTreeSet::from([Color::Black]),
            3,
            3,
        ),
        (
            "RAV-ORDRUUN-COMMANDO",
            "Ordruun Commando",
            ManaCost::with_colors(3, [Color::Red]),
            BTreeSet::from([Color::Red]),
            4,
            1,
        ),
        (
            "RAV-CIVIC-WAYFINDER",
            "Civic Wayfinder",
            ManaCost::with_colors(2, [Color::Green]),
            BTreeSet::from([Color::Green]),
            2,
            2,
        ),
        (
            "RAV-DOWSING-SHAMAN",
            "Dowsing Shaman",
            ManaCost::with_colors(2, [Color::Green, Color::Green]),
            BTreeSet::from([Color::Green]),
            3,
            4,
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
fn public_easy_creature_wave_two_scenarios_cover_casting_stack_zones_and_base_pt() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_easy_white_blue_creatures_wave_two",
        "rav_easy_blue_black_creatures_wave_two",
        "rav_easy_red_creatures_wave_two",
        "rav_easy_green_creatures_wave_two",
        "rav_easy_blue_red_creatures_wave_two",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
