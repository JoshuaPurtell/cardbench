//! Public contract for the remaining bounded RAV creature-chassis batch.
//!
//! These compatibility definitions deliberately expose normal casting and base
//! characteristics only. No printed activated, triggered, evasion, token, or
//! combat behavior is represented by this test or the executable slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Explicit base-fact matrix is intentionally audit-friendly.
fn second_creature_chassis_batch_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();
    let expected = [(
        "RAV-LORE-BROKER",
        "Lore Broker",
        ManaCost::with_colors(1, [Color::Blue]),
        BTreeSet::from([Color::Blue]),
        1,
        2,
    )];

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

    let sagittars = definitions
        .iter()
        .find(|definition| definition.id == "RAV-SELESNYA-SAGITTARS")
        .expect("Selesnya Sagittars definition exists");
    assert_eq!(sagittars.name, "Selesnya Sagittars");
    assert_eq!(
        sagittars.mana_cost,
        ManaCost::with_colors(3, [Color::Green, Color::White])
    );
    assert_eq!(
        sagittars.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(sagittars.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((sagittars.power, sagittars.toughness), (Some(2), Some(5)));
    assert_eq!(sagittars.keywords, [Keyword::Reach]);
    assert_eq!(
        sagittars.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "reach",
            "tap-damage-attacking-or-blocking-creature",
        ]
    );
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
