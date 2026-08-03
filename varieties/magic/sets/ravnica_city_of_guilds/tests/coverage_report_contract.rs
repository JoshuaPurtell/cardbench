use std::collections::BTreeSet;

use cardbench_magic_rav::{
    CardSemanticStatus, RAV_FULL_FIDELITY_DEFINITION_IDS, rav_main_set_catalog,
};

#[test]
fn coverage_categories_partition_every_printing_and_unique_name() {
    let full_ids = RAV_FULL_FIDELITY_DEFINITION_IDS
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut full_names = BTreeSet::new();
    let mut partial_names = BTreeSet::new();
    let mut catalog_only_names = BTreeSet::new();
    let mut full_printings = 0;
    let mut partial_printings = 0;
    let mut catalog_only_printings = 0;

    for card in rav_main_set_catalog() {
        match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { definition_id }
                if full_ids.contains(definition_id) =>
            {
                full_names.insert(card.name);
                full_printings += 1;
            }
            CardSemanticStatus::ExecutableCompatibilitySlice { .. } => {
                partial_names.insert(card.name);
                partial_printings += 1;
            }
            CardSemanticStatus::CatalogOnly { .. } => {
                catalog_only_names.insert(card.name);
                catalog_only_printings += 1;
            }
        }
    }

    assert_eq!(full_names.len(), 291);
    assert_eq!(full_printings, 306);
    assert_eq!(partial_names.len(), 0);
    assert_eq!(partial_printings, 0);
    assert_eq!(catalog_only_names.len(), 0);
    assert_eq!(catalog_only_printings, 0);
    assert_eq!(
        full_names.len() + partial_names.len() + catalog_only_names.len(),
        291
    );
    assert_eq!(
        full_printings + partial_printings + catalog_only_printings,
        306
    );
}

#[test]
fn every_positive_manifest_definition_declares_full_fidelity() {
    let definitions = cardbench_magic_rav::card_definitions();
    let manifest = RAV_FULL_FIDELITY_DEFINITION_IDS
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        definitions.len(),
        manifest.len(),
        "definition/manifest count drift"
    );

    let mut missing_markers = Vec::new();
    let mut stale_labels = Vec::new();
    for id in manifest {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .unwrap_or_else(|| panic!("positive manifest names missing definition {id}"));
        if definition.supported_rules.first().copied() != Some("full-rules-fidelity") {
            missing_markers.push(id);
        }
        if definition
            .supported_rules
            .iter()
            .any(|rule| rule.contains("compatibility") || rule.contains("unsupported"))
        {
            stale_labels.push(id);
        }
    }
    assert!(
        missing_markers.is_empty(),
        "missing fidelity markers: {missing_markers:?}"
    );
    assert!(
        stale_labels.is_empty(),
        "stale compatibility labels: {stale_labels:?}"
    );
}
