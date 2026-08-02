//! Red discovery contract for Leashling's hand-to-library activation cost.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn leashling_has_its_full_artifact_creature_definition() {
    let leashling = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LEASHLING")
        .expect("Leashling definition exists");

    assert_eq!(leashling.name, "Leashling");
    assert_eq!(leashling.mana_cost, ManaCost::new(6));
    assert_eq!(
        leashling.card_types,
        BTreeSet::from([CardType::Artifact, CardType::Creature])
    );
    assert_eq!((leashling.power, leashling.toughness), (Some(3), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&leashling.id));
    assert!(
        leashling
            .supported_rules
            .contains(&"hand-card-top-library-cost-return-source-owner-hand")
    );
}
