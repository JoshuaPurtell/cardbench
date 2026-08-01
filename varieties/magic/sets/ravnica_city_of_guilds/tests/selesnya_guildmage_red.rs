//! Red regression for Selesnya Guildmage's hybrid and activated rules.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, HybridManaSymbol, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn selesnya_guildmage_has_hybrid_cost_centaur_and_anthem_rules() {
    let guildmage = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELESNYA-GUILDMAGE")
        .expect("Selesnya Guildmage definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&guildmage.id));
    assert_eq!(guildmage.name, "Selesnya Guildmage");
    assert_eq!(
        guildmage.mana_cost,
        ManaCost::with_hybrid(
            0,
            [],
            [
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
            ],
        )
    );
    assert_eq!(guildmage.colors, BTreeSet::from([Color::Green, Color::White]));
    assert_eq!(guildmage.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((guildmage.power, guildmage.toughness), (Some(2), Some(2)));
    assert!(
        guildmage
            .supported_rules
            .contains(&"activated-green-centaur-token")
    );
    assert!(
        guildmage
            .supported_rules
            .contains(&"activated-controller-creature-anthem")
    );
}
