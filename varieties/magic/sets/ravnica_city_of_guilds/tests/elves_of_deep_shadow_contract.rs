//! Red-first public contract for complete RAV Elves of Deep Shadow support.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaAbilityOutput, ManaCost};
use cardbench_magic_rav::{card_definitions, rav_mana_ability_bindings};

#[test]
fn elves_of_deep_shadow_has_its_public_card_and_black_mana_binding() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("complete Elves of Deep Shadow definition");
    assert_eq!(definition.name, "Elves of Deep Shadow");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(0, [Color::Green]));
    assert_eq!(definition.colors, BTreeSet::from([Color::Green]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(definition.power, Some(1));
    assert_eq!(definition.toughness, Some(1));

    let binding = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("complete Elves of Deep Shadow mana binding");
    assert!(binding.ability.tap_cost);
    assert_eq!(binding.ability.output, ManaAbilityOutput::Fixed(Color::Black));
    assert_eq!(binding.ability.amount, 1);
    assert_eq!(binding.ability.life_payment, None);
}
