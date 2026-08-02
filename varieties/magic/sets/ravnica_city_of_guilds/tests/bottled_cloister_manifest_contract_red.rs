use cardbench_magic_rav::{CardSemanticStatus, RAV_FULL_FIDELITY_DEFINITION_IDS, rav_main_set_catalog};

#[test]
fn bottled_cloister_promotion_is_catalog_executable_and_manifested() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-BOTTLED-CLOISTER"));
    let card = rav_main_set_catalog()
        .into_iter()
        .find(|card| card.name == "Bottled Cloister")
        .expect("Bottled Cloister catalog entry");
    assert!(matches!(
        card.semantic_status,
        CardSemanticStatus::ExecutableCompatibilitySlice { .. }
    ));
}
