//! Red discovery contract for Szadek's bounded static Flying slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn szadek_exposes_static_flying_slice() {
    let szadek = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SZADEK")
        .expect("Szadek definition exists");
    assert_eq!(szadek.name, "Szadek, Lord of Secrets");
    assert_eq!(
        szadek.mana_cost,
        ManaCost::with_colors(3, [Color::Blue, Color::Blue, Color::Black, Color::Black])
    );
    assert_eq!(szadek.colors, BTreeSet::from([Color::Blue, Color::Black]));
    assert_eq!(szadek.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((szadek.power, szadek.toughness), (Some(5), Some(5)));
    assert_eq!(szadek.keywords, [Keyword::Flying]);
    assert!(szadek.supported_rules.contains(&"flying"));
}
