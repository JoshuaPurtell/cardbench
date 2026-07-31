//! Public contract for the first easy RAV creature-chassis coverage wave.
//!
//! These definitions intentionally implement normal casting and printed base
//! characteristics only. Printed triggers, activated abilities, evasion, and
//! combat restrictions stay outside the executable compatibility boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Explicit base-fact matrix is intentionally audit-friendly.
fn easy_creature_wave_one_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();
    let expected = [
        (
            "RAV-CERULEAN-SPHINX",
            "Cerulean Sphinx",
            ManaCost::with_colors(4, [Color::Blue, Color::Blue]),
            BTreeSet::from([Color::Blue]),
            5,
            5,
        ),
        (
            "RAV-HUNTED-PHANTASM",
            "Hunted Phantasm",
            ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            BTreeSet::from([Color::Blue]),
            4,
            6,
        ),
        (
            "RAV-VEDALKEN-DISMISSER",
            "Vedalken Dismisser",
            ManaCost::with_colors(5, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            2,
            2,
        ),
        (
            "RAV-ZEPHYR-SPIRIT",
            "Zephyr Spirit",
            ManaCost::with_colors(5, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            0,
            6,
        ),
        (
            "RAV-SADISTIC-AUGERMAGE",
            "Sadistic Augermage",
            ManaCost::with_colors(2, [Color::Black]),
            BTreeSet::from([Color::Black]),
            3,
            1,
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

    let oaf = definitions
        .iter()
        .find(|definition| definition.id == "RAV-INDENTURED-OAF")
        .expect("Indentured Oaf definition exists");
    assert_eq!(
        oaf.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "prevent-damage-from-red-sources"
        ]
    );

    let moloch = definitions
        .iter()
        .find(|definition| definition.id == "RAV-TORPID-MOLOCH")
        .expect("Torpid Moloch definition exists");
    assert_eq!(moloch.name, "Torpid Moloch");
    assert_eq!(moloch.mana_cost, ManaCost::with_colors(0, [Color::Red]));
    assert_eq!(moloch.colors, BTreeSet::from([Color::Red]));
    assert_eq!(moloch.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((moloch.power, moloch.toughness), (Some(3), Some(2)));
    assert_eq!(
        moloch.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "sacrifice-three-lands-remove-defender"
        ]
    );

    let sphinx = definitions
        .iter()
        .find(|definition| definition.id == "RAV-BELLTOWER-SPHINX")
        .expect("Belltower Sphinx definition exists");
    assert_eq!(sphinx.name, "Belltower Sphinx");
    assert_eq!(sphinx.mana_cost, ManaCost::with_colors(4, [Color::Blue]));
    assert_eq!(sphinx.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(sphinx.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((sphinx.power, sphinx.toughness), (Some(2), Some(5)));
    assert_eq!(
        sphinx.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );

    let griffin = definitions
        .iter()
        .find(|definition| definition.id == "RAV-SCREECHING-GRIFFIN")
        .expect("Screeching Griffin definition exists");
    assert_eq!(griffin.name, "Screeching Griffin");
    assert_eq!(griffin.mana_cost, ManaCost::with_colors(3, [Color::White]));
    assert_eq!(griffin.colors, BTreeSet::from([Color::White]));
    assert_eq!(griffin.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((griffin.power, griffin.toughness), (Some(2), Some(2)));
    assert_eq!(
        griffin.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );

    let drake = definitions
        .iter()
        .find(|definition| definition.id == "RAV-TATTERED-DRAKE")
        .expect("Tattered Drake definition exists");
    assert_eq!(drake.name, "Tattered Drake");
    assert_eq!(drake.mana_cost, ManaCost::with_colors(4, [Color::Blue]));
    assert_eq!(drake.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(drake.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((drake.power, drake.toughness), (Some(2), Some(2)));
    assert_eq!(
        drake.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
}

#[test]
fn easy_creature_wave_one_has_deterministic_public_scenarios() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_easy_white_creature_chassis",
        "rav_easy_blue_sphinx_chassis",
        "rav_belltower_sphinx_flying_compatibility",
        "rav_easy_blue_phantasm_drake_chassis",
        "rav_easy_blue_late_creature_chassis",
        "rav_easy_black_red_creature_chassis",
        "rav_torpid_moloch_defender_compatibility",
        "rav_screeching_griffin_flying_compatibility",
        "rav_tattered_drake_flying_compatibility",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
