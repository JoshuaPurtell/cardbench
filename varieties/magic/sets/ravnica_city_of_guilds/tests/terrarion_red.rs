//! Red regression for the public Terrarion semantic slice.
//!
//! The green milestone must model its entry replacement, paid sacrificed
//! mana ability, explicit two-color allocation, and resulting graveyard
//! trigger through generic engine substrates.

use cardbench_magic_engine::{CardType, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn terrarion_is_full_fidelity_colorless_artifact() {
    let terrarion = definition("RAV-TERRARION");
    assert_eq!(terrarion.mana_cost, ManaCost::new(1));
    assert_eq!(terrarion.card_types, [CardType::Artifact].into_iter().collect());
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&terrarion.id),
        "Terrarion must be explicitly promoted only with its complete public behavior"
    );
}
