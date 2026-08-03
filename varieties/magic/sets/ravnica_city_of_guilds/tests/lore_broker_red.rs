//! Red discovery contract for Lore Broker's simultaneous draw/discard activation.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn lore_broker_requires_each_player_draw_then_simultaneous_private_discard_choices() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LORE-BROKER")
        .expect("Lore Broker definition exists");

    assert_eq!(definition.name, "Lore Broker");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue])
    );
    assert_eq!(definition.colors, [Color::Blue].into());
    assert_eq!(definition.card_types, [CardType::Creature].into());
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(2)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    for rule in [
        "tap-activated-each-player-draws-one",
        "simultaneous-private-each-player-discard-after-draw",
    ] {
        assert!(definition.supported_rules.contains(&rule), "missing {rule}");
    }
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-LORE-BROKER"
            && binding.ability.id == "each-player-draws-then-discards"
            && binding.ability.tap_cost
    }));
}
