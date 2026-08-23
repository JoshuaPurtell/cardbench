//! Red discovery contract for Svogthos's animated graveyard-count land body.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_mana_ability_bindings,
};

#[test]
fn svogthos_requires_dynamic_graveyard_animation_and_colorless_mana() {
    let svogthos = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SVOGTHOS-THE-RESTLESS-TOMB")
        .expect("Svogthos definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&svogthos.id));
    assert_eq!(svogthos.name, "Svogthos, the Restless Tomb");
    assert_eq!(svogthos.mana_cost, ManaCost::new(0));
    assert_eq!(svogthos.colors, BTreeSet::new());
    assert_eq!(svogthos.mana_colors, BTreeSet::from([Color::Colorless]));
    assert_eq!(svogthos.card_types, BTreeSet::from([CardType::Land]));
    assert_eq!((svogthos.power, svogthos.toughness), (None, None));
    assert_eq!(
        executable_definition_id_for_collector(283),
        Ok("RAV-SVOGTHOS-THE-RESTLESS-TOMB")
    );
    assert!(
        svogthos
            .supported_rules
            .contains(&"activated-dynamic-graveyard-creature-animation")
    );
    assert!(rav_mana_ability_bindings().iter().any(|binding| {
        binding.card_definition == svogthos.id && binding.ability.id == "produce-colorless"
    }));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == svogthos.id
            && binding.ability.id == "animate-self-from-controller-graveyard-creatures"
    }));
}
