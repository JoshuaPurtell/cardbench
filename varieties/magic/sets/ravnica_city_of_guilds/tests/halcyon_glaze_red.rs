//! Red discovery contract for Halcyon Glaze's creature-spell animation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn halcyon_glaze_requires_creature_spell_self_animation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HALCYON-GLAZE")
        .expect("Halcyon Glaze definition exists");

    assert_eq!(definition.name, "Halcyon Glaze");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(2, [Color::Blue]));
    assert_eq!(definition.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Enchantment]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Halcyon Glaze cannot be complete while its creature-spell animation is absent"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"creature-spell-triggered-self-animation"),
        "Halcyon Glaze must expose its temporary self-animation rule"
    );
}
