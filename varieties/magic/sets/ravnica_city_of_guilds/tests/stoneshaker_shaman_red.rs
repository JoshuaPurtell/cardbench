//! Red regression for Stoneshaker Shaman's active-player end-step sacrifice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

const STONESHAKER_SHAMAN: &str = "RAV-STONESHAKER-SHAMAN";

#[test]
fn stoneshaker_shaman_requires_each_end_step_active_player_untapped_land_sacrifice() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == STONESHAKER_SHAMAN)
        .expect("Stoneshaker Shaman definition exists");
    assert_eq!(definition.name, "Stoneshaker Shaman");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(2, [Color::Red]));
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(1)));
    assert!(
        definition
            .supported_rules
            .contains(&"each-end-step-active-player-sacrifices-untapped-land"),
        "the active player's untapped-land sacrifice is not a bounded chassis detail"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Stoneshaker Shaman needs the target-free active-end-step trigger before promotion"
    );
}
