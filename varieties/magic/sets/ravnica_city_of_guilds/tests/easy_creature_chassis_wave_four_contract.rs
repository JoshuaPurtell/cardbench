//! Public contract for the fourth bounded RAV creature-chassis batch.
//!
//! These compatibility definitions deliberately expose normal casting and base
//! characteristics only. Goblin Fire Fiend is separately audited by its full
//! fidelity contract and is excluded from the bounded matrix below. Woebringer
//! Demon separately exposes its static Flying compatibility slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Explicit base-fact matrix is intentionally audit-friendly.
fn fourth_creature_chassis_batch_is_exactly_bounded_to_public_base_facts() {
    let definitions = card_definitions();
    let expected = [
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

    let excruciator = definitions
        .iter()
        .find(|definition| definition.id == "RAV-EXCRUCIATOR")
        .expect("Excruciator definition exists");
    assert_eq!(
        excruciator.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "damage-cannot-be-prevented"
        ]
    );

    let brute = definitions
        .iter()
        .find(|definition| definition.id == "RAV-SELL-SWORD-BRUTE")
        .expect("Sell-Sword Brute definition exists");
    assert_eq!(
        brute.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "dies-deal-two-to-controller"
        ]
    );

    let demon = definitions
        .iter()
        .find(|definition| definition.id == "RAV-WOEBRINGER-DEMON")
        .expect("Woebringer Demon remains in the fourth chassis wave");
    assert_eq!(demon.name, "Woebringer Demon");
    assert_eq!(
        demon.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Black])
    );
    assert_eq!(demon.colors, BTreeSet::from([Color::Black]));
    assert_eq!(demon.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((demon.power, demon.toughness), (Some(4), Some(4)));
    assert_eq!(demon.keywords, [cardbench_magic_engine::Keyword::Flying]);
    assert_eq!(
        demon.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
    assert!(demon.effects.is_empty());
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
        "rav_goblin_fire_fiend_haste_compatibility",
        "rav_woebringer_demon_flying_compatibility",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
