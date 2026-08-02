//! Red discovery contract for Dark Heart of the Wood's typed Forest-sacrifice activation.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dark_heart_of_the_wood_requires_its_exact_enchantment_definition() {
    let heart = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DARK-HEART-OF-THE-WOOD")
        .expect("Dark Heart of the Wood definition exists");

    assert_eq!(heart.name, "Dark Heart of the Wood");
    assert_eq!(heart.mana_cost, ManaCost::with_colors(0, [Color::Green]));
    assert_eq!(heart.card_types, [CardType::Enchantment].into_iter().collect());
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&heart.id),
        "Dark Heart is full only when its typed Forest-sacrifice activation is represented"
    );
}
