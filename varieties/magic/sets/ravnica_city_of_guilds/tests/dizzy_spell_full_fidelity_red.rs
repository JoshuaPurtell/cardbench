use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dizzy_spell_is_not_left_as_a_partial_slice() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIZZY-SPELL")
        .expect("Dizzy Spell definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Dizzy Spell has typed modifier and Transmute scenarios but is not full-fidelity"
    );
    assert!(definition.supported_rules.contains(&"full-rules-fidelity"));
}
