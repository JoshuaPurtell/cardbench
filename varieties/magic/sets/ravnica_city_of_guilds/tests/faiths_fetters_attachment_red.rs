//! Red regression for the missing general Aura attachment/suppression substrate.

use cardbench_magic_engine::{CardType, ManaCost, Color};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn faiths_fetters_requires_general_permanent_attachment_semantics() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FAITHS-FETTERS")
        .expect("Faith's Fetters definition exists");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(3, [Color::White]));
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.supported_rules.contains(&"aura-enchant-permanent"));
    assert!(definition.supported_rules.contains(&"etb-gain-four-life"));
    assert!(
        definition
            .supported_rules
            .contains(&"attached-permanent-combat-and-activation-restriction")
    );
}
