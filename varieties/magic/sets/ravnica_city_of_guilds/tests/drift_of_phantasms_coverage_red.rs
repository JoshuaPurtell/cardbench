//! Red regression for Drift of Phantasms' complete static and Transmute slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn drift_of_phantasms_requires_stack_backed_transmute_for_full_fidelity() {
    let drift = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DRIFT-OF-PHANTASMS")
        .expect("Drift of Phantasms definition exists");

    assert_eq!(drift.name, "Drift of Phantasms");
    assert_eq!(drift.mana_cost, ManaCost::with_colors(2, [Color::Blue]));
    assert_eq!(drift.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(drift.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(drift.power, Some(0));
    assert_eq!(drift.toughness, Some(5));
    assert_eq!(
        drift.keywords,
        vec![
            Keyword::Defender,
            Keyword::Flying,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
        ],
        "Defender, Flying, and Transmute have shared engine support"
    );
    assert_eq!(drift.effects, Vec::new());
    assert_eq!(
        drift.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "flying",
            "transmute",
        ],
        "there is no remaining printed behavior to keep this definition bounded"
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&drift.id));
}
