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

    assert_eq!(full_names.len(), 96);
    assert_eq!(partial_names.len(), 58);
    assert_eq!(catalog_only_names.len(), 137);
    assert_eq!(full_printings, 111);
    assert_eq!(partial_printings, 58);
    assert_eq!(catalog_only_printings, 137);
    assert_eq!(
        full_names.len() + partial_names.len() + catalog_only_names.len(),
        291
    );
    assert_eq!(
        full_printings + partial_printings + catalog_only_printings,
        306
    );
}
