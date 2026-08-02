//! Red discovery contract for Voyager Staff's linked-exile lifecycle.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn voyager_staff_requires_exact_linked_exile_definition() {
    let staff = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VOYAGER-STAFF")
        .expect("Voyager Staff definition exists");
    assert_eq!(staff.name, "Voyager Staff");
    assert_eq!(staff.mana_cost, ManaCost::new(1));
    assert_eq!(staff.colors, BTreeSet::<Color>::new());
    assert_eq!(staff.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&staff.id));
    assert!(
        staff
            .supported_rules
            .contains(&"sacrifice-linked-exile-target-creature-until-end-step")
    );
}

#[test]
fn voyager_staff_is_currently_catalog_only() {
    assert!(executable_definition_id_for_collector(274).is_err());
    assert!(
        !card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-VOYAGER-STAFF")
    );
}
