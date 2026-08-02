//! Red discovery contract for Duskmantle, House of Shadow's mill activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{ActivatedAbility, CardType, Color, Effect, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    CatalogResolutionError, RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions,
    executable_definition_id_for_collector, rav_activated_ability_bindings,
};

#[test]
fn duskmantle_requires_exact_land_and_target_player_mill_activation() {
    let duskmantle = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DUSKMANTLE-HOUSE-OF-SHADOW")
        .expect("Duskmantle, House of Shadow definition exists");
    assert_eq!(duskmantle.name, "Duskmantle, House of Shadow");
    assert_eq!(duskmantle.mana_cost, ManaCost::new(0));
    assert_eq!(duskmantle.colors, BTreeSet::<Color>::new());
    assert_eq!(duskmantle.card_types, BTreeSet::from([CardType::Land]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&duskmantle.id));
    assert!(duskmantle
        .supported_rules
        .contains(&"tap-target-player-mill-one"));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == duskmantle.id
            && binding.ability
                == ActivatedAbility {
                    id: "tap-target-player-mill-one",
                    mana_cost: ManaCost::new(0),
                    tap_cost: true,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: false,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![TargetRequirement::Player],
                    effects: vec![Effect::MillTargetPlayer { count: 1 }],
                }
    }));
}

#[test]
fn duskmantle_stays_catalog_only_without_the_typed_mill_activation() {
    assert_eq!(
        executable_definition_id_for_collector(277),
        Err(CatalogResolutionError::CapabilityGap {
            collector_number: 277,
            name: "Duskmantle, House of Shadow",
            capability_gap: "card-specific-rules-not-implemented",
        })
    );
    assert!(
        !card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-DUSKMANTLE-HOUSE-OF-SHADOW"),
        "catalog-only land has no blank executable fallback"
    );
}
