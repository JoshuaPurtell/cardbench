//! Red regression for Surveilling Sprite's Flying and dies-draw behavior.

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn surveilling_sprite_requires_flying_and_a_dies_draw_trigger() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SURVEILLING-SPRITE")
        .expect("Surveilling Sprite definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!(definition.keywords, [Keyword::Flying]);
    assert!(
        definition
            .supported_rules
            .contains(&"dies-draw-controller")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
