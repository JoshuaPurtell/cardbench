//! Red regression for the token/counter replacement substrate used by Doubling Season.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn doubling_season_has_a_full_fidelity_replacement_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DOUBLING-SEASON")
        .expect("Doubling Season definition exists");

    assert_eq!(definition.name, "Doubling Season");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(4, [Color::Green]));
    assert_eq!(definition.card_types, [CardType::Enchantment].into_iter().collect());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"controlled-token-creation-replacement")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"controlled-plus-one-counter-replacement")
    );
}
