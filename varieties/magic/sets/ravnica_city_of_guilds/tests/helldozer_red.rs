//! Red discovery probe for Helldozer's complete land-destruction activation.

use cardbench_magic_engine::{Color, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn helldozer_requires_exact_black_activation_and_nonbasic_untap_behavior() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HELLDOZER")
        .expect("Helldozer definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Helldozer is full only once its post-destruction condition is modeled"
    );
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Black, Color::Black])
    );
    assert_eq!((definition.power, definition.toughness), (Some(6), Some(5)));
    let ability = &rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == definition.id && binding.ability.id == "destroy-target-land"
        })
        .expect("Helldozer land-destruction binding exists")
        .ability;
    assert_eq!(
        ability.mana_cost,
        ManaCost::with_colors(0, [Color::Black, Color::Black, Color::Black])
    );
    assert!(ability.tap_cost);
    assert_eq!(ability.targets, [TargetRequirement::Land]);
}
