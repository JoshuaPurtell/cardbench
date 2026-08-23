//! Red discovery contract for Sunhome's missing executable land substrate.

use cardbench_magic_rav::executable_definition_id_for_collector;

#[test]
fn sunhome_is_not_silently_catalog_only_when_its_typed_abilities_are_claimed() {
    assert_eq!(
        executable_definition_id_for_collector(282),
        Ok("RAV-SUNHOME-FORTRESS"),
        "Sunhome should expose its typed colorless mana and Double Strike activation"
    );
}
