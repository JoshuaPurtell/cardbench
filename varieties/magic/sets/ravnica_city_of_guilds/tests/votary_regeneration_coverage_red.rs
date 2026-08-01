//! Red regression for Votary of the Conclave's targeted regeneration.

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn votary_requires_regeneration_instead_of_a_false_vigilance_promotion() {
    let votary = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VOTARY-OF-THE-CONCLAVE")
        .expect("Votary of the Conclave definition exists");

    assert_eq!(votary.name, "Votary of the Conclave");
    assert_eq!(votary.mana_cost, ManaCost::with_colors(0, [Color::White]));
    assert_eq!(
        votary.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert!(
        votary.supported_rules.contains(&"regeneration"),
        "Votary's activated regeneration ability is missing from supported_rules"
    );
    assert!(
        !votary.keywords.contains(&Keyword::Vigilance),
        "this probe prevents approximating regeneration as Vigilance"
    );
}
