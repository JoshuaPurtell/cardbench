//! Red regression for the source-only Strands of Undeath Aura chassis.

use cardbench_magic_engine::{CardType, Color, Effect, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn strands_of_undeath_has_its_exact_static_aura_chassis_without_unimplemented_abilities() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STRANDS-OF-UNDEATH")
        .expect("Strands of Undeath definition exists");

    assert_eq!(definition.name, "Strands of Undeath");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(3, [Color::Black]));
    assert_eq!(definition.colors, [Color::Black].into_iter().collect());
    assert_eq!(definition.card_types, [CardType::Enchantment].into_iter().collect());
    assert_eq!(
        definition.effects,
        [Effect::AttachSourceAndModifyTargetPt {
            power: 0,
            toughness: 0,
        }]
    );
    assert_eq!(
        definition.supported_rules,
        [
            "aura-static-attachment-only",
            "etb-discard-and-enchanted-regeneration-not-implemented",
        ]
    );
}
