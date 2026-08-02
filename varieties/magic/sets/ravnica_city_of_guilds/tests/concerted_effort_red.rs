//! Red discovery contract for Concerted Effort's upkeep keyword sharing.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn concerted_effort_requires_its_upkeep_shared_keyword_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CONCERTED-EFFORT")
        .expect("Concerted Effort definition exists");

    assert_eq!(definition.name, "Concerted Effort");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Enchantment]));
    assert!(
        definition
            .supported_rules
            .contains(&"upkeep-controller-creature-keyword-sharing")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
