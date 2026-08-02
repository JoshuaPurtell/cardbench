use cardbench_magic_rav::{
    CardSemanticStatus, CatalogResolutionError, RAV_MAIN_SET_BASIC_LAND_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    executable_definition_id_for_collector, rav_main_set_catalog,
    validate_rav_catalog_executable_boundary, validate_rav_main_set_catalog,
};

#[test]
fn checked_in_public_manifest_covers_every_rav_main_set_printing() {
    validate_rav_main_set_catalog().expect("complete RAV catalog");
    let catalog = rav_main_set_catalog();
    assert_eq!(catalog.len(), RAV_MAIN_SET_EXPECTED_PRINTING_COUNT);
    assert_eq!(
        catalog
            .iter()
            .map(|card| card.name)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
    );
    assert_eq!(
        catalog
            .iter()
            .filter(|card| matches!(
                card.name,
                "Plains" | "Island" | "Swamp" | "Mountain" | "Forest"
            ))
            .count(),
        RAV_MAIN_SET_BASIC_LAND_PRINTING_COUNT
    );
}

#[test]
fn every_catalog_record_declares_and_enforces_its_semantic_boundary() {
    validate_rav_catalog_executable_boundary().expect("catalog/executable boundary");
    for card in rav_main_set_catalog() {
        match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
                assert_eq!(
                    executable_definition_id_for_collector(card.collector_number),
                    Ok(definition_id),
                    "RAV #{} must resolve only to its declared definition",
                    card.collector_number
                );
            }
            CardSemanticStatus::CatalogOnly { capability_gap } => {
                assert!(!capability_gap.is_empty());
                assert!(matches!(
                    executable_definition_id_for_collector(card.collector_number),
                    Err(CatalogResolutionError::CapabilityGap { .. })
                ));
            }
        }
    }
}

#[test]
fn executable_slice_size_is_explicit_and_does_not_masquerade_as_set_coverage() {
    let catalog = rav_main_set_catalog();
    let executable_printings = catalog
        .iter()
        .filter(|card| card.semantic_status.is_executable())
        .count();
    let executable_names = catalog
        .iter()
        .filter(|card| card.semantic_status.is_executable())
        .map(|card| card.name)
        .collect::<std::collections::BTreeSet<_>>();
    let catalog_only_names = catalog
        .iter()
        .filter(|card| !card.semantic_status.is_executable())
        .map(|card| card.name)
        .collect::<std::collections::BTreeSet<_>>();

    // Twenty executable basic-land printings collapse to five names. The
    // two hundred and eighteen executable nonbasic names include the positive
    // full-fidelity manifest entries; all others remain deliberately bounded
    // compatibility slices. The full-fidelity Clinging Darkness and
    // Moldervine Cloak entries use the bounded static-modifier Aura substrate;
    // regeneration remains independently capability-gated.
    assert_eq!(executable_printings, 238);
    assert_eq!(executable_names.len(), 223);
    assert_eq!(catalog_only_names.len(), 68);
    assert!(executable_names.is_disjoint(&catalog_only_names));
}

#[test]
fn clinging_darkness_resolves_to_its_executable_aura_definition() {
    assert_eq!(
        executable_definition_id_for_collector(80),
        Ok("RAV-CLINGING-DARKNESS")
    );
}

#[test]
fn auratouched_mage_resolves_to_its_bounded_aura_search_definition() {
    assert_eq!(
        executable_definition_id_for_collector(1),
        Ok("RAV-AURATOUCHED-MAGE")
    );
}

#[test]
fn disembowel_resolves_to_its_bounded_chosen_x_definition() {
    assert_eq!(
        executable_definition_id_for_collector(85),
        Ok("RAV-DISEMBOWEL")
    );
}
