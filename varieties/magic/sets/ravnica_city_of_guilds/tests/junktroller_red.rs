//! Red discovery contract for Junktroller's public graveyard-to-library move.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Keyword, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn junktroller_requires_any_graveyard_card_to_owners_library_bottom_behavior() {
    let junktroller = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-JUNKTROLLER")
        .expect("Junktroller definition exists");

    assert_eq!(junktroller.name, "Junktroller");
    assert_eq!(junktroller.mana_cost, ManaCost::new(4));
    assert_eq!(junktroller.colors, BTreeSet::new());
    assert_eq!(
        junktroller.card_types,
        BTreeSet::from([CardType::Artifact, CardType::Creature])
    );
    assert_eq!(junktroller.power, Some(0));
    assert_eq!(junktroller.toughness, Some(6));
    assert_eq!(junktroller.keywords, vec![Keyword::Defender]);
    assert!(
        junktroller
            .supported_rules
            .contains(&"tap-target-graveyard-card-to-owners-library-bottom")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&junktroller.id));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-JUNKTROLLER"
            && binding.ability.id == "tap-target-graveyard-card-to-owners-library-bottom"
    }));
}
