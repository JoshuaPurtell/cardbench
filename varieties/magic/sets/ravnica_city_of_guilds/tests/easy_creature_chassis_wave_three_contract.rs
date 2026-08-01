//! Public contract for the third bounded RAV creature-chassis batch.
//!
//! These compatibility definitions deliberately expose ordinary colored-cost
//! casting and printed base characteristics only. Card-specific activated,
//! triggered, evasion, library, and combat behavior remains outside the
//! executable boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Explicit base-fact matrix is intentionally audit-friendly.
fn third_creature_chassis_batch_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();
    let expected = [
        (
            "RAV-PRIMORDIAL-SAGE",
            "Primordial Sage",
            ManaCost::with_colors(4, [Color::Green, Color::Green]),
            BTreeSet::from([Color::Green]),
            4,
            5,
        ),
        (
            "RAV-TRANSLUMINANT",
            "Transluminant",
            ManaCost::with_colors(1, [Color::Green]),
            BTreeSet::from([Color::Green]),
            2,
            2,
        ),
        (
            "RAV-TROPHY-HUNTER",
            "Trophy Hunter",
            ManaCost::with_colors(2, [Color::Green]),
            BTreeSet::from([Color::Green]),
            2,
            3,
        ),
        (
            "RAV-URSAPINE",
            "Ursapine",
            ManaCost::with_colors(3, [Color::Green, Color::Green]),
            BTreeSet::from([Color::Green]),
            3,
            3,
        ),
        (
            "RAV-VINELASHER-KUDZU",
            "Vinelasher Kudzu",
            ManaCost::with_colors(1, [Color::Green]),
            BTreeSet::from([Color::Green]),
            1,
            1,
        ),
        (
            "RAV-WOODWRAITH-CORRUPTER",
            "Woodwraith Corrupter",
            ManaCost::with_colors(3, [Color::Black, Color::Black, Color::Green]),
            BTreeSet::from([Color::Black, Color::Green]),
            3,
            6,
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
            "{id} must not present unsupported card-specific behavior"
        );
        assert!(definition.keywords.is_empty(), "{id}");
        assert!(definition.effects.is_empty(), "{id}");
    }
}

#[test]
fn public_third_chassis_scenarios_cover_costs_stack_zones_and_base_pt() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_wave_three_green_creature_chassis",
        "rav_wave_three_guild_creature_chassis",
        "rav_wave_three_boros_golgari_creature_chassis",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
