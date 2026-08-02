//! Red discovery contract for Glare of Subdual's creature-tap activations.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn glare_of_subdual_has_both_printed_creature_tap_activations() {
    let glare = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GLARE-OF-SUBDUAL")
        .expect("Glare of Subdual definition exists");
    assert_eq!(glare.name, "Glare of Subdual");
    assert_eq!(
        glare.mana_cost,
        ManaCost::with_colors(2, [Color::Green, Color::White])
    );
    assert_eq!(glare.colors, BTreeSet::from([Color::Green, Color::White]));
    assert_eq!(glare.card_types, BTreeSet::from([CardType::Enchantment]));
    assert_eq!((glare.power, glare.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&glare.id));
    assert!(
        glare
            .supported_rules
            .contains(&"tap-untapped-controlled-creature-tap-target-creature")
    );
    assert!(
        glare
            .supported_rules
            .contains(&"tap-untapped-controlled-creature-prevent-all-combat-damage")
    );
}

#[test]
fn glare_of_subdual_binds_one_creature_cost_to_each_activated_ability() {
    let bindings = rav_activated_ability_bindings();
    let tap_target = bindings
        .iter()
        .find(|binding| {
            binding.card_definition == "RAV-GLARE-OF-SUBDUAL"
                && binding.ability.id == "tap-target-creature"
        })
        .expect("Glare of Subdual tap activation exists");
    assert_eq!(tap_target.ability.mana_cost, ManaCost::new(0));
    assert!(!tap_target.ability.tap_cost);
    assert_eq!(tap_target.ability.additional_tap_creatures, 1);
    assert_eq!(tap_target.ability.targets, vec![TargetRequirement::Creature]);

    let prevent_combat = bindings
        .iter()
        .find(|binding| {
            binding.card_definition == "RAV-GLARE-OF-SUBDUAL"
                && binding.ability.id == "prevent-all-combat-damage"
        })
        .expect("Glare of Subdual combat-prevention activation exists");
    assert_eq!(prevent_combat.ability.mana_cost, ManaCost::new(0));
    assert!(!prevent_combat.ability.tap_cost);
    assert_eq!(prevent_combat.ability.additional_tap_creatures, 1);
    assert!(prevent_combat.ability.targets.is_empty());
}
