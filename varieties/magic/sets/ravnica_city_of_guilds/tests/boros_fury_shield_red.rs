//! Red discovery contract for Boros Fury-Shield's conditional combat prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Game, ManaCost, PlayerId, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn boros_fury_shield_has_its_exact_conditional_combat_prevention_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BOROS-FURY-SHIELD")
        .expect("Boros Fury-Shield definition exists");
    assert_eq!(definition.name, "Boros Fury-Shield");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"prevent-target-creatures-combat-damage-and-red-spend-controller-damage")
    );
}

#[test]
fn boros_fury_shield_is_available_for_its_targeted_combat_prevention_trace() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    game.add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("attacking creature setup");
    game.add_card(PlayerId(0), "RAV-BOROS-FURY-SHIELD", Zone::Hand)
        .expect("Boros Fury-Shield setup");
}
