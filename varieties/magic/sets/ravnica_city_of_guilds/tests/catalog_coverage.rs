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
    // two hundred and eighty-three executable nonbasic names include the positive
    // full-fidelity manifest entries; all others remain deliberately bounded
    // compatibility slices. The full-fidelity Clinging Darkness and
    // Moldervine Cloak entries use the bounded static-modifier Aura substrate;
    // regeneration remains independently capability-gated.
    assert_eq!(executable_printings, 303);
    assert_eq!(executable_names.len(), 288);
    assert_eq!(catalog_only_names.len(), 3);
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
fn auratouched_mage_resolves_to_its_full_private_aura_search_definition() {
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

#[test]
fn consult_the_necrosages_resolves_to_its_full_modal_definition() {
    assert_eq!(
        executable_definition_id_for_collector(199),
        Ok("RAV-CONSULT-THE-NECROSAGES")
    );
}

#[test]
fn junktroller_resolves_to_its_full_graveyard_target_definition() {
    assert_eq!(
        executable_definition_id_for_collector(264),
        Ok("RAV-JUNKTROLLER")
    );
}

#[test]
fn leashling_resolves_to_its_full_hand_library_cost_definition() {
    assert_eq!(
        executable_definition_id_for_collector(265),
        Ok("RAV-LEASHLING")
    );
}

#[test]
fn crown_of_convergence_resolves_to_its_full_controller_library_definition() {
    assert_eq!(
        executable_definition_id_for_collector(258),
        Ok("RAV-CROWN-OF-CONVERGENCE")
    );
}

#[test]
fn bloodletter_quill_resolves_to_its_full_counter_definition() {
    assert_eq!(
        executable_definition_id_for_collector(254),
        Ok("RAV-BLOODLETTER-QUILL")
    );
}

#[test]
fn dimir_cutpurse_resolves_to_its_full_combat_player_trigger_definition() {
    assert_eq!(
        executable_definition_id_for_collector(201),
        Ok("RAV-DIMIR-CUTPURSE")
    );
}

#[test]
fn dimir_doppelganger_resolves_to_its_full_graveyard_copy_definition() {
    assert_eq!(
        executable_definition_id_for_collector(202),
        Ok("RAV-DIMIR-DOPPELGANGER")
    );
}

#[test]
fn sisters_of_stone_death_resolves_to_its_full_source_linked_combat_definition() {
    assert_eq!(
        executable_definition_id_for_collector(231),
        Ok("RAV-SISTERS-OF-STONE-DEATH")
    );
}

#[test]
fn chorus_of_the_conclave_resolves_to_its_full_optional_creature_payment_definition() {
    assert_eq!(
        executable_definition_id_for_collector(195),
        Ok("RAV-CHORUS-OF-THE-CONCLAVE")
    );
}

#[test]
fn bloodbond_march_resolves_to_its_full_any_player_creature_cast_definition() {
    assert_eq!(
        executable_definition_id_for_collector(192),
        Ok("RAV-BLOODBOND-MARCH")
    );
}
