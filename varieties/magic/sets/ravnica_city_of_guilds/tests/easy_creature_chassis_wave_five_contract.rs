//! Public contract for the first collector-range creature batch.
//!
//! The generic chassis entries expose only normal casting and base
//! characteristics. Drift of Phantasms separately records its bounded
//! Defender/immediate-Transmute compatibility slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Explicit base-fact matrix is audit-friendly.
fn first_range_creature_chassis_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();
    let expected = [
        (
            "RAV-SANDSOWER",
            "Sandsower",
            ManaCost::with_colors(3, [Color::White]),
            BTreeSet::from([Color::White]),
            1,
            3,
        ),
        (
            "RAV-VOTARY-OF-THE-CONCLAVE",
            "Votary of the Conclave",
            ManaCost::with_colors(0, [Color::White]),
            BTreeSet::from([Color::White]),
            1,
            1,
        ),
        (
            "RAV-DRAKE-FAMILIAR",
            "Drake Familiar",
            ManaCost::with_colors(1, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            2,
            1,
        ),
        (
            "RAV-ETHEREAL-USHER",
            "Ethereal Usher",
            ManaCost::with_colors(5, [Color::Blue]),
            BTreeSet::from([Color::Blue]),
            2,
            3,
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

    let divebomber = definitions
        .iter()
        .find(|definition| definition.id == "RAV-DIVEBOMBER-GRIFFIN")
        .expect("Divebomber Griffin definition exists");
    assert_eq!(divebomber.name, "Divebomber Griffin");
    assert_eq!(
        divebomber.mana_cost,
        ManaCost::with_colors(3, [Color::White, Color::White])
    );
    assert_eq!(divebomber.colors, BTreeSet::from([Color::White]));
    assert_eq!(divebomber.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((divebomber.power, divebomber.toughness), (Some(3), Some(2)));
    assert_eq!(divebomber.keywords, [Keyword::Flying]);
    assert!(divebomber.effects.is_empty());
    assert_eq!(
        divebomber.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );

    let drift = definitions
        .iter()
        .find(|definition| definition.id == "RAV-DRIFT-OF-PHANTASMS")
        .expect("Drift of Phantasms definition exists");
    assert_eq!(drift.name, "Drift of Phantasms");
    assert_eq!(drift.mana_cost, ManaCost::with_colors(2, [Color::Blue]));
    assert_eq!(drift.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(drift.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((drift.power, drift.toughness), (Some(0), Some(5)));
    assert_eq!(
        drift.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "immediate-hand-zone-transmute-compatibility",
        ]
    );

    let grozoth = definitions
        .iter()
        .find(|definition| definition.id == "RAV-GROZOTH")
        .expect("Grozoth definition exists");
    assert_eq!(grozoth.name, "Grozoth");
    assert_eq!(
        grozoth.mana_cost,
        ManaCost::with_colors(6, [Color::Blue, Color::Blue, Color::Blue])
    );
    assert_eq!(grozoth.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(grozoth.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((grozoth.power, grozoth.toughness), (Some(9), Some(9)));
    assert_eq!(
        grozoth.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "immediate-hand-zone-transmute-compatibility",
        ]
    );
}

#[test]
fn first_range_creature_chassis_has_deterministic_public_scenarios() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_easy_white_creature_chassis_wave_five",
        "rav_divebomber_griffin_flying_compatibility",
        "rav_easy_blue_creature_chassis_wave_five",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
