use cardbench_magic_engine::Color;
use cardbench_magic_rav::card_definitions;

#[test]
fn barbarian_riftcutter_declares_its_sacrifice_land_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BARBARIAN-RIFTCUTTER")
        .expect("Barbarian Riftcutter exists");
    assert!(definition.colors.contains(&Color::Red));
    assert!(definition
        .supported_rules
        .contains(&"sacrifice-source-destroy-land"));
}
