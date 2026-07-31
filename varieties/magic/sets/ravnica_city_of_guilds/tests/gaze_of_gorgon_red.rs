//! Public fail-closed contract for a removed false executable semantic slice.
//!
//! This records CardBench-authored semantic facts from the public RAV audit;
//! it does not retain card rules text or artwork.

use cardbench_magic_rav::{
    CatalogResolutionError, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn gaze_of_gorgon_stays_catalog_only_until_its_required_substrates_exist() {
    assert_eq!(
        executable_definition_id_for_collector(246),
        Err(CatalogResolutionError::CapabilityGap {
            collector_number: 246,
            name: "Gaze of the Gorgon",
            capability_gap: "regeneration-and-end-of-combat-block-history-destruction-not-implemented",
        })
    );
    assert!(
        !card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-GAZE-OF-THE-GORGON"),
        "the prior unrelated temporary modifier cannot be an executable fallback"
    );
}
