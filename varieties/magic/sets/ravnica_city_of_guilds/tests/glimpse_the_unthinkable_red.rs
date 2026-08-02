//! Red discovery contract for Glimpse the Unthinkable's absent typed mill slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Effect, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    CatalogResolutionError, RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions,
    executable_definition_id_for_collector,
};

#[test]
fn glimpse_requires_its_exact_target_player_mill_definition() {
    let glimpse = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GLIMPSE-THE-UNTHINKABLE")
        .expect("Glimpse the Unthinkable definition exists");

    assert_eq!(glimpse.name, "Glimpse the Unthinkable");
    assert_eq!(
        glimpse.mana_cost,
        ManaCost::with_colors(0, [Color::Blue, Color::Black])
    );
    assert_eq!(glimpse.colors, BTreeSet::from([Color::Blue, Color::Black]));
    assert_eq!(glimpse.card_types, BTreeSet::from([CardType::Sorcery]));
    assert_eq!(
        glimpse.effects,
        [Effect::MillTargetPlayer { count: 10 }]
    );
    assert_eq!(
        glimpse.effects[0].target_requirement(),
        Some(TargetRequirement::Player)
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&glimpse.id));
    assert!(glimpse.supported_rules.contains(&"targeted-mill-ten"));
}

#[test]
fn glimpse_stays_catalog_only_without_its_exact_mill_definition() {
    assert_eq!(
        executable_definition_id_for_collector(208),
        Err(CatalogResolutionError::CapabilityGap {
            collector_number: 208,
            name: "Glimpse the Unthinkable",
            capability_gap: "card-specific-rules-not-implemented",
        })
    );
    assert!(
        !card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-GLIMPSE-THE-UNTHINKABLE"),
        "a catalog-only card must not become a blank executable fallback"
    );
}
