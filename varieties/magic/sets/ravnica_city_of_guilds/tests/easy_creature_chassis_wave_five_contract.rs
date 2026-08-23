//! Public contract for the first collector-range creature batch.
//!
//! The former generic chassis entries are promoted only when their complete
//! card-specific contracts become public. Sandsower, Divebomber Griffin,
//! Drake Familiar, Drift of Phantasms, and Ethereal Usher have separate
//! full-fidelity contracts; Grozoth is likewise promoted through its
//! stack-backed transmute/search substrate.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Explicit base-fact matrix is audit-friendly.
fn first_range_creature_promotions_leave_only_explicit_bounded_chassis() {
    let definitions = card_definitions();

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
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "flying",
            "transmute",
        ]
    );

    let usher = definitions
        .iter()
        .find(|definition| definition.id == "RAV-ETHEREAL-USHER")
        .expect("Ethereal Usher definition exists");
    assert_eq!(usher.name, "Ethereal Usher");
    assert_eq!(usher.mana_cost, ManaCost::with_colors(5, [Color::Blue]));
    assert_eq!(usher.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(usher.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((usher.power, usher.toughness), (Some(2), Some(3)));
    assert_eq!(
        usher.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-target-unblockable-until-end-of-turn",
            "transmute",
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
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "optional-private-multi-card-mana-value-search",
            "stack-backed-private-transmute",
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
