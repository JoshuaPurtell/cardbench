//! Red regression for the source-only Firemane Angel card definition.

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn firemane_angel_has_its_exact_static_chassis_without_claiming_unported_text() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FIREMANE-ANGEL")
        .expect("Firemane Angel definition exists");

    assert_eq!(definition.name, "Firemane Angel");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Red, Color::White, Color::White])
    );
    assert_eq!(
        definition.colors,
        [Color::Red, Color::White].into_iter().collect()
    );
    assert_eq!(definition.card_types, [CardType::Creature].into_iter().collect());
    assert_eq!(definition.power, Some(4));
    assert_eq!(definition.toughness, Some(3));
    assert_eq!(definition.keywords, [Keyword::Flying, Keyword::FirstStrike]);
    assert_eq!(
        definition.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "first-strike",
        ]
    );
}
