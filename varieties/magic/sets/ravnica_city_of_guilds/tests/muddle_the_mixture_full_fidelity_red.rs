use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn muddle_the_mixture_is_not_left_as_a_partial_slice() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MUDDLE-THE-MIXTURE")
        .expect("Muddle the Mixture definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Muddle has typed counterspell and Transmute scenarios but is not full-fidelity"
    );
    assert!(definition.supported_rules.contains(&"full-rules-fidelity"));
}
