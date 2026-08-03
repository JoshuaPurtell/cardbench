//! Red discovery contract for Phytohydra's self-damage replacement.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_damage_replacement_effect_bindings,
};

#[test]
fn phytohydra_requires_a_self_damage_prevention_and_counter_replacement() {
    let phytohydra = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PHYTOHYDRA")
        .expect("Phytohydra definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&phytohydra.id));
    assert_eq!(phytohydra.name, "Phytohydra");
    assert_eq!(
        phytohydra.mana_cost,
        ManaCost::with_colors(2, [Color::Green, Color::White, Color::White])
    );
    assert_eq!(
        phytohydra.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(phytohydra.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((phytohydra.power, phytohydra.toughness), (Some(1), Some(1)));
    assert!(
        phytohydra
            .supported_rules
            .contains(&"self-damage-prevention-plus-one-counters")
    );
    assert_eq!(
        executable_definition_id_for_collector(218),
        Ok("RAV-PHYTOHYDRA")
    );
    assert!(rav_damage_replacement_effect_bindings()
        .iter()
        .any(|binding| binding.source_definition == phytohydra.id));
}
