//! Red coverage probe for Keening Banshee's ETB creature modifier.

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn keening_banshee_exposes_its_complete_etb_slice() {
    let banshee = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-KEENING-BANSHEE")
        .expect("Keening Banshee definition exists");
    assert_eq!(
        banshee.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Black])
    );
    assert_eq!(
        banshee.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((banshee.power, banshee.toughness), (Some(2), Some(2)));
    assert!(banshee.keywords.contains(&Keyword::Flying));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&banshee.id));
    assert!(
        banshee
            .supported_rules
            .contains(&"enter-battlefield-targeted-minus-two-minus-two")
    );
}
