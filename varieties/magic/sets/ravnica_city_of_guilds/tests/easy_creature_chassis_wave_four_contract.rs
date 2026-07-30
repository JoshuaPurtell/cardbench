//! Public contract for the fourth bounded RAV creature-chassis batch.
//!
//! These compatibility definitions deliberately expose normal casting and base
//! characteristics only. Their printed activations, triggers, evasion, combat
//! restrictions, and damage-prevention exception remain unimplemented.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Explicit base-fact matrix is intentionally audit-friendly.
fn fourth_creature_chassis_batch_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();
    let expected = [
        (
            "RAV-WOEBRINGER-DEMON",
            "Woebringer Demon",
            ManaCost::with_colors(3, [Color::Black, Color::Black]),
            BTreeSet::from([Color::Black]),
            4,
            4,
        ),
        (
            "RAV-THOUGHTPICKER-WITCH",
            "Thoughtpicker Witch",
            ManaCost::with_colors(0, [Color::Black]),
            BTreeSet::from([Color::Black]),
            1,
            1,
        ),
        (
            "RAV-UNDERCITY-SHADE",
            "Undercity Shade",
            ManaCost::with_colors(4, [Color::Black]),
            BTreeSet::from([Color::Black]),
            1,
            1,
        ),
        (
            "RAV-VINDICTIVE-MOB",
            "Vindictive Mob",
            ManaCost::with_colors(4, [Color::Black, Color::Black]),
            BTreeSet::from([Color::Black]),
            5,
            5,
        ),
        (
            "RAV-BARBARIAN-RIFTCUTTER",
            "Barbarian Riftcutter",
            ManaCost::with_colors(4, [Color::Red]),
            BTreeSet::from([Color::Red]),
            3,
            3,
        ),
        (
            "RAV-EXCRUCIATOR",
            "Excruciator",
            ManaCost::with_colors(6, [Color::Red, Color::Red]),
            BTreeSet::from([Color::Red]),
            7,
            7,
        ),
        (
            "RAV-GOBLIN-FIRE-FIEND",
            "Goblin Fire Fiend",
            ManaCost::with_colors(3, [Color::Red]),
            BTreeSet::from([Color::Red]),
            1,
            1,
        ),
        (
            "RAV-SELL-SWORD-BRUTE",
            "Sell-Sword Brute",
            ManaCost::with_colors(1, [Color::Red]),
            BTreeSet::from([Color::Red]),
            2,
            2,
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
fn public_fourth_chassis_scenarios_cover_costs_stack_zones_and_base_pt() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_demon_witch_creature_chassis",
        "rav_shade_mob_creature_chassis",
        "rav_riftcutter_excruciator_creature_chassis",
        "rav_fire_fiend_brute_creature_chassis",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
