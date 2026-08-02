//! Red discovery contract for Crown of Convergence's complete static slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn crown_of_convergence_requires_its_owned_top_library_static_layer_and_rotation() {
    let crown = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CROWN-OF-CONVERGENCE")
        .expect("Crown of Convergence definition exists");

    assert_eq!(crown.name, "Crown of Convergence");
    assert_eq!(crown.mana_cost, ManaCost::new(2));
    assert_eq!(crown.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(
        crown
            .supported_rules
            .contains(&"controller-top-library-revealed")
    );
    assert!(
        crown
            .supported_rules
            .contains(&"top-creature-shared-color-creatures-plus-one-plus-one")
    );
    assert!(
        crown
            .supported_rules
            .contains(&"green-white-rotate-controller-library-top-to-bottom")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&crown.id));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-CROWN-OF-CONVERGENCE"
            && binding.ability.id == "green-white-rotate-controller-library-top-to-bottom"
    }));
}
