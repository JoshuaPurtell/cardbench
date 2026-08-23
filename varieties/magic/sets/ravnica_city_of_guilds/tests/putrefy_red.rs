//! Red regression for Putrefy's absent artifact-or-creature destruction slice.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn putrefy_is_not_silently_catalog_only_when_its_target_and_regeneration_rules_are_claimed() {
    let spell = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PUTREFY")
        .expect("Putrefy definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&spell.id));
    assert_eq!(spell.card_types, [CardType::Instant].into_iter().collect());
    assert_eq!(
        spell.colors,
        [Color::Black, Color::Green].into_iter().collect()
    );
    assert_eq!(
        spell.mana_cost,
        ManaCost::with_colors(0, [Color::Black, Color::Green])
    );
    assert!(
        spell
            .supported_rules
            .contains(&"artifact-or-creature-destruction-no-regeneration")
    );
}
