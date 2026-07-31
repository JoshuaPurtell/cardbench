//! Ignored full-fidelity boundary probe for Belltower Sphinx.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Belltower Sphinx's damage-triggered behavior is not implemented"]
fn belltower_sphinx_requires_its_damage_trigger_for_full_fidelity() {
    let sphinx = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BELLTOWER-SPHINX")
        .expect("Belltower Sphinx definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sphinx.id));
}
