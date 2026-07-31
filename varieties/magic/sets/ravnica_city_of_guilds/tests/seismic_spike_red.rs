//! Red discovery probe for Seismic Spike's complete front-face effect.

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn seismic_spike_requires_land_destruction_and_two_red_mana() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SEISMIC-SPIKE")
        .expect("Seismic Spike definition exists");
    assert_eq!(definition.mana_cost.generic, 3);
    assert_eq!(definition.mana_cost.colored, vec![Color::Red]);
    assert_eq!(definition.card_types, [CardType::Sorcery].into_iter().collect());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.supported_rules.contains(&"destroy-target-land"));
    assert!(definition.supported_rules.contains(&"add-two-red-mana"));
}
