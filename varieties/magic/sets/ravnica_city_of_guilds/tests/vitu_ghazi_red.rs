//! Red regression for Vitu-Ghazi's typed land and Saproling activation.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn vitu_ghazi_has_colorless_mana_and_a_green_saproling_activation() {
    let land = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VITU-GHAZI")
        .expect("Vitu-Ghazi definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&land.id));
    assert_eq!(land.name, "Vitu-Ghazi, the City-Tree");
    assert_eq!(land.card_types, [CardType::Land].into_iter().collect());
    assert_eq!(land.mana_colors, [Color::Colorless].into_iter().collect());
    assert!(land.supported_rules.contains(&"activated-green-saproling-token"));
    assert_eq!(ManaCost::with_colors(2, [Color::Green, Color::White]).mana_value(), 4);
}
