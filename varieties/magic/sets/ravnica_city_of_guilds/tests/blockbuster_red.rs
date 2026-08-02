//! Red discovery contract for Blockbuster's absent stack-backed activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{ActivatedAbility, CardType, Color, Effect, ManaCost};
use cardbench_magic_rav::{
    CatalogResolutionError, RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions,
    executable_definition_id_for_collector, rav_activated_ability_bindings,
};

#[test]
fn blockbuster_requires_exact_artifact_and_global_damage_activation() {
    let blockbuster = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BLOCKBUSTER")
        .expect("Blockbuster definition exists");
    assert_eq!(blockbuster.name, "Blockbuster");
    assert_eq!(blockbuster.mana_cost, ManaCost::new(4));
    assert_eq!(blockbuster.colors, BTreeSet::<Color>::new());
    assert_eq!(blockbuster.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&blockbuster.id));
    assert!(blockbuster
        .supported_rules
        .contains(&"tap-global-creature-and-player-damage"));

    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == blockbuster.id
            && binding.ability
                == ActivatedAbility {
                    id: "tap-global-creature-and-player-damage",
                    mana_cost: ManaCost::new(3),
                    tap_cost: true,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: false,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![],
                    effects: vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 3 }],
                }
    }));
}

#[test]
fn blockbuster_stays_catalog_only_without_its_typed_activation() {
    assert_eq!(
        executable_definition_id_for_collector(115),
        Err(CatalogResolutionError::CapabilityGap {
            collector_number: 115,
            name: "Blockbuster",
            capability_gap: "card-specific-rules-not-implemented",
        })
    );
    assert!(
        !card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-BLOCKBUSTER"),
        "catalog-only Blockbuster has no blank executable fallback"
    );
}
