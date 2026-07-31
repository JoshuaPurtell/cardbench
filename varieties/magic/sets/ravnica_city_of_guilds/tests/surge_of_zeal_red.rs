//! Red discovery contract for Surge of Zeal's Radiance haste grant.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn surge_of_zeal_requires_its_radiance_haste_effect() {
    let surge = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SURGE-OF-ZEAL")
        .expect("Surge of Zeal definition exists");
    assert_eq!(surge.name, "Surge of Zeal");
    assert_eq!(surge.mana_cost, ManaCost::with_colors(0, [Color::Red]));
    assert_eq!(surge.colors, [Color::Red].into_iter().collect());
    assert_eq!(surge.card_types, [CardType::Instant].into_iter().collect());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&surge.id));
    assert!(surge.supported_rules.contains(&"radiance-grant-haste"));
}
