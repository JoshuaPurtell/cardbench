//! Public static-keyword contract for fully represented Carven Caryatid.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn carven_caryatid_exposes_its_supported_defender_slice() {
    let caryatid = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CARVEN-CARYATID")
        .expect("Carven Caryatid definition exists");

    assert_eq!(caryatid.name, "Carven Caryatid");
    assert_eq!(
        caryatid.mana_cost,
        ManaCost::with_colors(1, [Color::Green, Color::Green])
    );
    assert_eq!(caryatid.colors, BTreeSet::from([Color::Green]));
    assert_eq!(caryatid.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((caryatid.power, caryatid.toughness), (Some(2), Some(5)));
    assert_eq!(caryatid.keywords, [Keyword::Defender]);
    assert!(caryatid.effects.is_empty());
    assert!(caryatid.supported_rules.contains(&"full-rules-fidelity"));
    assert!(
        caryatid
            .supported_rules
            .contains(&"enter-the-battlefield-draw")
    );
}
