//! Red regression for Grayscaled Gharial's omitted static landwalk behavior.

use cardbench_magic_engine::{BasicLandType, Keyword};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn grayscaled_gharial_requires_islandwalk_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GRAYSCALED-GHARIAL")
        .expect("Grayscaled Gharial definition exists");

    assert!(definition
        .keywords
        .contains(&Keyword::Landwalk(BasicLandType::Island)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
