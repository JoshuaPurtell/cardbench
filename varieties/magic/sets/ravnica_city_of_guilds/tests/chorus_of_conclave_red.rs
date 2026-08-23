//! Red regression for Chorus of the Conclave's battlefield cast-cost effect.

use cardbench_magic_engine::{BasicLandType, CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn chorus_of_the_conclave_requires_live_optional_creature_cast_mana_and_entry_counters() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CHORUS-OF-THE-CONCLAVE")
        .expect("Chorus of the Conclave definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(definition.name, "Chorus of the Conclave");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Green, Color::Green, Color::White, Color::White])
    );
    assert_eq!(definition.colors, [Color::Green, Color::White].into());
    assert_eq!(definition.card_types, [CardType::Creature].into());
    assert_eq!(definition.power, Some(3));
    assert_eq!(definition.toughness, Some(8));
    assert!(
        definition
            .keywords
            .contains(&Keyword::Landwalk(BasicLandType::Forest))
    );
    assert!(
        definition
            .supported_rules
            .contains(&"battlefield-optional-any-mana-creature-cast-entry-counters")
    );
}
