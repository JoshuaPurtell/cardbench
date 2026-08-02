//! Red discovery contract for Light of Sanction's static friendly-fire prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Game, ManaCost, PlayerId, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn light_of_sanction_has_its_static_controller_relative_prevention_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LIGHT-OF-SANCTION")
        .expect("Light of Sanction definition exists");
    assert_eq!(definition.name, "Light of Sanction");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::White, Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"static-prevent-friendly-source-damage")
    );
}

#[test]
fn light_of_sanction_is_available_to_supply_its_static_prevention_binding() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    game.add_card(PlayerId(0), "RAV-LIGHT-OF-SANCTION", Zone::Battlefield)
        .expect("Light of Sanction setup");
}
