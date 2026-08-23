//! Fail-closed boundary for the public RAV vanilla/chassis audit.
//!
//! The accompanying catalog provenance records the public printing audit. This
//! contract deliberately stores only identifiers and `CardBench` semantic facts,
//! never card rules prose or source responses.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn only_publicly_verified_ability_free_rav_chassis_have_positive_fidelity_markers() {
    let definitions = card_definitions();
    let expected = [
        (
            "RAV-WATCHWOLF",
            ManaCost::with_colors(0, [Color::Green, Color::White]),
            [Color::Green, Color::White]
                .into_iter()
                .collect::<BTreeSet<_>>(),
            [CardType::Creature].into_iter().collect::<BTreeSet<_>>(),
            (3, 3),
        ),
        (
            "RAV-GLASS-GOLEM",
            ManaCost::new(5),
            BTreeSet::new(),
            [CardType::Artifact, CardType::Creature]
                .into_iter()
                .collect::<BTreeSet<_>>(),
            (6, 2),
        ),
    ];

    for (id, mana_cost, colors, card_types, power_toughness) in expected {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .expect("audited chassis definition exists");
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id),
            "only audited ability-free chassis may carry this positive claim"
        );
        assert_eq!(definition.supported_rules[0], "full-rules-fidelity", "{id}");
        assert_eq!(definition.mana_cost, mana_cost, "{id}");
        assert_eq!(definition.colors, colors, "{id}");
        assert_eq!(definition.card_types, card_types, "{id}");
        assert_eq!(
            (definition.power, definition.toughness),
            (Some(power_toughness.0), Some(power_toughness.1)),
            "{id}"
        );
        assert!(definition.keywords.is_empty(), "{id}");
        assert!(definition.effects.is_empty(), "{id}");
    }
}

#[test]
fn bounded_chassis_are_never_promoted_solely_because_their_engine_vectors_are_empty() {
    let bounded = card_definitions()
        .into_iter()
        .filter(|definition| {
            definition.supported_rules == ["colored-cost-casting", "base-characteristics"]
                && definition.keywords.is_empty()
                && definition.effects.is_empty()
        })
        .collect::<Vec<_>>();
    assert!(
        bounded.is_empty(),
        "all empty-vector RAV chassis are promoted"
    );
    for definition in bounded {
        assert!(
            !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
            "{} requires a public ability-completeness audit before promotion",
            definition.id
        );
    }
}
