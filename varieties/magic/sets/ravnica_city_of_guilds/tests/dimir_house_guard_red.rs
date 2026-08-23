//! Public static-characteristics regression for Dimir House Guard.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dimir_house_guard_exposes_its_static_fear_and_transmute_slice() {
    let guard = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-HOUSE-GUARD")
        .expect("Dimir House Guard definition exists");

    assert_eq!(guard.name, "Dimir House Guard");
    assert_eq!(guard.mana_cost, ManaCost::with_colors(3, [Color::Black]));
    assert_eq!(guard.colors, BTreeSet::from([Color::Black]));
    assert_eq!(guard.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((guard.power, guard.toughness), (Some(2), Some(3)));
    assert_eq!(
        guard.keywords,
        [
            Keyword::Fear,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Black, Color::Black])),
        ]
    );
    assert_eq!(
        guard.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "fear",
            "stack-backed-private-transmute",
            "sacrifice-creature-regenerate",
        ]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&guard.id),
        "the full-fidelity activation is covered by its dedicated stack regression"
    );
}
