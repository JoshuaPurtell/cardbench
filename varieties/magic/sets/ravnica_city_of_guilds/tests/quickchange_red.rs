//! Red regression for Quickchange's policy-selected color layer and draw.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn quickchange_requires_target_color_replacement_and_controller_draw() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-QUICKCHANGE")
        .expect("Quickchange definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Instant].into_iter().collect()
    );
    assert!(
        definition
            .supported_rules
            .contains(&"policy-chosen-target-color-replacement-until-end-of-turn")
    );
    assert!(definition.supported_rules.contains(&"controller-draw"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
