//! Red discovery contract for the missing Consult the Necrosages card slice.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn consult_the_necrosages_requires_a_cast_selected_modal_target_player_effect() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CONSULT-THE-NECROSAGES")
        .expect("Consult the Necrosages definition exists");

    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Consult cannot be full-fidelity until its cast-selected draw-two or discard-two mode is represented"
    );
    assert!(definition.supported_rules.contains(&"full-rules-fidelity"));
    assert!(
        definition
            .supported_rules
            .contains(&"caster-selected-modal-target-player-draw-or-discard"),
        "the card must retain a policy-selected mode rather than a deterministic default"
    );
}
