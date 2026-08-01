//! Red regression for Hunted Troll's complete executable rules slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn hunted_troll_has_its_entry_tokens_and_regeneration_binding() {
    let troll = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-TROLL")
        .expect("Hunted Troll definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&troll.id));
    assert_eq!(troll.name, "Hunted Troll");
    assert_eq!(
        troll.mana_cost,
        ManaCost::with_colors(2, [Color::Green, Color::Green])
    );
    assert_eq!(troll.colors, BTreeSet::from([Color::Green]));
    assert_eq!(troll.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((troll.power, troll.toughness), (Some(8), Some(4)));
    assert!(
        troll
            .supported_rules
            .contains(&"etb-targeted-opponent-flying-faerie-tokens")
    );
    assert!(troll.supported_rules.contains(&"regeneration"));
}
