//! Public fail-closed contract for a removed false executable semantic slice.
//!
//! This records CardBench-authored semantic facts from the public RAV audit;
//! it does not retain card rules text or artwork.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, HybridManaSymbol, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn gaze_of_gorgon_requires_regeneration_and_delayed_combat_history_destruction() {
    let gaze = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GAZE-OF-THE-GORGON")
        .expect("Gaze of the Gorgon definition exists");

    assert_eq!(gaze.name, "Gaze of the Gorgon");
    assert_eq!(
        gaze.mana_cost,
        ManaCost::with_hybrid(
            3,
            [],
            [HybridManaSymbol {
                first: Color::Black,
                second: Color::Green,
            }],
        )
    );
    assert_eq!(gaze.colors, BTreeSet::from([Color::Black, Color::Green]));
    assert_eq!(gaze.card_types, BTreeSet::from([CardType::Instant]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&gaze.id));
    assert!(
        gaze.supported_rules
            .contains(&"targeted-regeneration-shield")
    );
    assert!(
        gaze.supported_rules
            .contains(&"delayed-end-of-combat-block-history-destruction")
    );
}

#[test]
fn gaze_of_gorgon_catalog_mapping_unblocks_only_the_typed_full_definition() {
    assert_eq!(
        executable_definition_id_for_collector(246),
        Ok("RAV-GAZE-OF-THE-GORGON")
    );
    assert!(
        card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-GAZE-OF-THE-GORGON"),
        "the catalog mapping must identify the typed full definition"
    );
}
