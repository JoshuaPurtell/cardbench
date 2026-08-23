//! Red regression for Cerulean Sphinx's omitted evasion and owner-library ability.

use cardbench_magic_engine::{Color, Keyword, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn cerulean_sphinx_requires_flying_and_owner_library_shuffle_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CERULEAN-SPHINX")
        .expect("Cerulean Sphinx definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Blue, Color::Blue])
    );
    assert!(definition.keywords.contains(&Keyword::Flying));
    assert!(
        definition
            .supported_rules
            .contains(&"activated-source-owner-library-shuffle")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-CERULEAN-SPHINX"
            && binding.ability.id == "shuffle-source-into-owner-library"
    }));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
