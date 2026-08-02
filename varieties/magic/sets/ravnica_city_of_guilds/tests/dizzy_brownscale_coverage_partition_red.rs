use std::collections::BTreeSet;

use cardbench_magic_rav::{CardSemanticStatus, RAV_FULL_FIDELITY_DEFINITION_IDS, rav_main_set_catalog};

#[test]
fn post_promotion_partition_matches_the_catalog() {
    let full_ids = RAV_FULL_FIDELITY_DEFINITION_IDS
        .into_iter()
        .collect::<BTreeSet<_>>();
    let partial_names = rav_main_set_catalog()
        .into_iter()
        .filter_map(|card| match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { definition_id }
                if !full_ids.contains(definition_id) => Some(card.name),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(partial_names.len(), 45);
}
