//! Red discovery contract for Tolsimir Wolfblood's two color-specific anthems
//! and named legendary Wolf token ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn tolsimir_wolfblood_requires_color_specific_anthems_and_voja_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TOLSIMIR-WOLFBLOOD")
        .expect("Tolsimir Wolfblood definition exists");

    assert_eq!(definition.name, "Tolsimir Wolfblood");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Green, Color::White])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(3), Some(4)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"other-green-and-white-creatures-get-plus-one-plus-one")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"tap-create-named-legendary-green-white-wolf-token")
    );
}
