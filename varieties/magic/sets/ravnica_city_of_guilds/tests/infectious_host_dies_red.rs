//! Red coverage probe for Infectious Host's dies trigger.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn infectious_host_exposes_its_targeted_dies_life_loss_slice() {
    let host = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INFECTIOUS-HOST")
        .expect("Infectious Host definition exists");
    assert_eq!(host.mana_cost, ManaCost::with_colors(1, [Color::Black]));
    assert_eq!(host.card_types, [CardType::Creature].into_iter().collect());
    assert_eq!((host.power, host.toughness), (Some(1), Some(1)));
    assert!(
        host.supported_rules
            .contains(&"dies-target-player-life-loss")
    );
}
