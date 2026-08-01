//! Red discovery contract for Gate Hound's Aura-conditioned static effect.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn gate_hound_requires_its_enchanted_controller_vigilance_static_effect() {
    let hound = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GATE-HOUND")
        .expect("Gate Hound definition exists");

    assert_eq!(hound.name, "Gate Hound");
    assert_eq!(hound.mana_cost, ManaCost::with_colors(2, [Color::White]));
    assert_eq!(hound.colors, BTreeSet::from([Color::White]));
    assert_eq!(hound.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((hound.power, hound.toughness), (Some(1), Some(1)));
    assert!(hound.keywords.is_empty());
    assert_eq!(hound.effects, []);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&hound.id));
    assert!(
        hound
            .supported_rules
            .contains(&"static-controller-vigilance-while-enchanted")
    );
}
