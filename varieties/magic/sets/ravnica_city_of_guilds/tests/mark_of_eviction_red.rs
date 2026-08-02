//! Red regression for Mark of Eviction's attached-source upkeep trigger.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn mark_of_eviction_requires_a_source_relative_enchanted_creature_return() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MARK-OF-EVICTION")
        .expect("Mark of Eviction definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(
        definition
            .supported_rules
            .contains(&"aura-enchant-creature-upkeep-return-enchanted-creature")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
