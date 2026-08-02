//! Red discovery contract for Spectral Searchlight.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn spectral_searchlight_requires_target_recipient_mana_choice_behavior() {
    let searchlight = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SPECTRAL-SEARCHLIGHT")
        .expect("Spectral Searchlight definition exists");

    assert_eq!(searchlight.name, "Spectral Searchlight");
    assert_eq!(searchlight.mana_cost, ManaCost::new(3));
    assert_eq!(searchlight.colors, BTreeSet::new());
    assert_eq!(searchlight.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(searchlight.keywords.is_empty());
    assert!(searchlight.effects.is_empty());
    assert!(
        searchlight
            .supported_rules
            .contains(&"target-player-chooses-one-color-mana")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&searchlight.id));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-SPECTRAL-SEARCHLIGHT"
            && binding.ability.id == "tap-target-player-chosen-color-mana"
    }));
}
