//! Red discovery contract for Bloodbond March's all-player creature-cast trigger.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn bloodbond_march_requires_captured_any_player_creature_cast_identity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BLOODBOND-MARCH")
        .expect("Bloodbond March definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(definition.name, "Bloodbond March");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Green])
    );
    assert_eq!(definition.colors, [Color::Black, Color::Green].into());
    assert_eq!(definition.card_types, [CardType::Enchantment].into());
    assert!(
        definition
            .supported_rules
            .contains(&"any-player-creature-cast-matching-graveyard-creature-return")
    );
}
