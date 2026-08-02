//! Red regression for Dream Leash's source-relative Aura control effect.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dream_leash_requires_a_permanent_aura_that_uses_its_current_controller() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DREAM-LEASH")
        .expect("Dream Leash definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Blue, Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(
        definition
            .supported_rules
            .contains(&"aura-enchant-permanent-source-controller-control")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
