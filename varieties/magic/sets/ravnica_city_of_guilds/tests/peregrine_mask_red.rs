//! Red discovery contract for Peregrine Mask's Equipment lifecycle.

use std::collections::BTreeSet;

use cardbench_magic_engine::{AttachmentKind, CardType, Color, ContinuousChange, Keyword, ManaCost};
use cardbench_magic_rav::{
    CatalogResolutionError, RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions,
    executable_definition_id_for_collector, rav_attachment_bindings,
};

#[test]
fn peregrine_mask_requires_exact_equipment_keyword_binding() {
    let mask = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PEREGRINE-MASK")
        .expect("Peregrine Mask definition exists");
    assert_eq!(mask.name, "Peregrine Mask");
    assert_eq!(mask.mana_cost, ManaCost::new(1));
    assert_eq!(mask.colors, BTreeSet::<Color>::new());
    assert_eq!(mask.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&mask.id));
    assert!(mask
        .supported_rules
        .contains(&"equipment-defender-flying-first-strike"));
    assert!(rav_attachment_bindings().iter().any(|binding| {
        binding.card_definition == mask.id
            && binding.kind == AttachmentKind::Equipment
            && binding.changes
                == vec![
                    ContinuousChange::AddKeyword(Keyword::Defender),
                    ContinuousChange::AddKeyword(Keyword::Flying),
                    ContinuousChange::AddKeyword(Keyword::FirstStrike),
                ]
    }));
}

#[test]
fn peregrine_mask_stays_catalog_only_without_equipment_binding() {
    assert_eq!(
        executable_definition_id_for_collector(268),
        Err(CatalogResolutionError::CapabilityGap {
            collector_number: 268,
            name: "Peregrine Mask",
            capability_gap: "card-specific-rules-not-implemented",
        })
    );
    assert!(!card_definitions()
        .iter()
        .any(|definition| definition.id == "RAV-PEREGRINE-MASK"));
}
