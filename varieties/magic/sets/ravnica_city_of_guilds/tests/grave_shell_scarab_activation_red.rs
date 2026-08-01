//! Red regression for Grave-Shell Scarab's sacrifice-to-draw activation.

use cardbench_magic_rav::{card_definitions, rav_activated_ability_bindings};

#[test]
fn grave_shell_scarab_requires_sacrifice_source_draw_activation() {
    let scarab = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GRAVE-SHELL-SCARAB")
        .expect("Grave-Shell Scarab is catalogued");
    assert!(
        scarab.supported_rules.contains(&"sacrifice-source-draw"),
        "Grave-Shell Scarab activation is missing from supported_rules"
    );
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == scarab.id)
        .expect("Grave-Shell Scarab activated binding");
    assert_eq!(binding.ability.id, "sacrifice-source-draw");
}
