//! Red discovery contract for Lurking Informant's private top-library choice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, HybridManaSymbol, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings,
};

#[test]
fn lurking_informant_requires_target_player_top_library_may_graveyard_activation() {
    let informant = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LURKING-INFORMANT")
        .expect("Lurking Informant definition exists");

    assert_eq!(informant.name, "Lurking Informant");
    assert_eq!(
        informant.mana_cost,
        ManaCost::with_hybrid(
            1,
            BTreeSet::new(),
            [HybridManaSymbol {
                first: Color::Blue,
                second: Color::Black,
            }],
        )
    );
    assert_eq!(
        informant.colors,
        BTreeSet::from([Color::Blue, Color::Black])
    );
    assert_eq!(informant.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(informant.power, Some(1));
    assert_eq!(informant.toughness, Some(2));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&informant.id));
    assert!(
        informant
            .supported_rules
            .contains(&"tap-two-target-player-private-top-library-may-graveyard")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == informant.id
            && binding.ability.id == "two-tap-target-player-top-library-may-graveyard"
            && binding.ability.mana_cost == ManaCost::new(2)
            && binding.ability.tap_cost
            && binding.ability.targets == vec![TargetRequirement::Player]
    }));
}

#[test]
fn lurking_informant_catalog_mapping_requires_the_typed_definition() {
    assert_eq!(
        executable_definition_id_for_collector(249),
        Ok("RAV-LURKING-INFORMANT")
    );
}
