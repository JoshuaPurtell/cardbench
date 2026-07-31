//! Full-fidelity probe retained red until Undercity Shade's activation exists.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Undercity Shade's temporary power/toughness activation is not implemented"]
fn undercity_shade_requires_activation_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-UNDERCITY-SHADE")
        .expect("Undercity Shade definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
