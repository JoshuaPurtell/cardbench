//! Ignored full-fidelity boundary probe for Drift of Phantasms.
//!
//! The represented Defender and immediate hand-zone Transmute slices are useful
//! compatibility coverage, but real Transmute is an activated ability with a
//! response window. The shared substrate resolves it immediately, so this
//! positive-manifest assertion must remain red until stack-backed activated
//! abilities exist.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
#[ignore = "Transmute is immediate rather than stack-backed, so full fidelity is not yet valid"]
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
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
        ],
        "Defender and Transmute already have shared engine support"
    );
    assert_eq!(drift.effects, Vec::new());
    assert_eq!(
        drift.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "transmute",
        ],
        "there is no remaining printed behavior to keep this definition bounded"
    );
}
