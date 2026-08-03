//! Red contract for Copy Enchantment's as-enters copy choice.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn copy_enchantment_needs_its_as_enters_enchantment_copy_choice() {
    let copy_enchantment = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-COPY-ENCHANTMENT")
        .expect("Copy Enchantment definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&copy_enchantment.id),
        "Copy Enchantment cannot be complete while its as-enters copy choice is absent"
    );
    assert_eq!(
        copy_enchantment.mana_cost,
        ManaCost::with_colors(2, [Color::Blue])
    );
    assert_eq!(copy_enchantment.card_types.len(), 1);
    assert!(copy_enchantment.card_types.contains(&CardType::Enchantment));
    assert!(
        copy_enchantment
            .supported_rules
            .contains(&"may-enter-as-copy-of-enchantment"),
        "Copy Enchantment must expose its optional as-enters copy rule"
    );
}
