//! Red discovery contract for Instill Furor's attached-creature end-step rule.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn instill_furor_requires_its_attacked_this_turn_sacrifice_rule() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INSTILL-FUROR")
        .expect("Instill Furor definition exists");

    assert_eq!(definition.name, "Instill Furor");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(1, [Color::Red]));
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"attached-creature-end-step-attack-sacrifice")
    );
}
