//! Red regression for the source-only Necromantic Thirst Aura chassis.

use cardbench_magic_engine::{CardType, Color, Effect, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn necromantic_thirst_has_its_exact_static_aura_chassis_without_unimplemented_trigger_claims() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NECROMANTIC-THIRST")
        .expect("Necromantic Thirst definition exists");

    assert_eq!(definition.name, "Necromantic Thirst");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Black])
    );
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
        ["aura-static-attachment-only", "combat-damage-trigger-not-implemented"]
    );
}
