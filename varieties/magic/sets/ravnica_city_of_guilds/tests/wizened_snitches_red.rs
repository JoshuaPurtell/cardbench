//! Red discovery contract for Wizened Snitches' public top-library visibility.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn wizened_snitches_has_its_global_top_library_visibility_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WIZENED-SNITCHES")
        .expect("Wizened Snitches definition exists");

    assert_eq!(definition.name, "Wizened Snitches");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Blue])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"global-static-top-library-visibility")
    );
}
