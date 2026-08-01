//! Red regression for the unported Clinging Darkness Aura.

use cardbench_magic_engine::{CardType, Color, Effect, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn clinging_darkness_has_its_exact_persistent_aura_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CLINGING-DARKNESS")
        .expect("Clinging Darkness definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(definition.name, "Clinging Darkness");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(1, [Color::Black]));
    assert_eq!(definition.colors, [Color::Black].into_iter().collect());
    assert_eq!(definition.card_types, [CardType::Enchantment].into_iter().collect());
    assert!(definition.keywords.is_empty());
    assert_eq!(
        definition.effects,
        [Effect::AttachSourceAndModifyTargetPt {
            power: -3,
            toughness: -1,
        }]
    );
}
