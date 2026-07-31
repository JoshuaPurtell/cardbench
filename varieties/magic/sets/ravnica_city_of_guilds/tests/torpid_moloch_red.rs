use cardbench_magic_rav::card_definitions;

#[test]
fn torpid_moloch_declares_its_three_land_defender_cost() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TORPID-MOLOCH")
        .expect("Torpid Moloch exists");
    assert!(definition
        .supported_rules
        .contains(&"sacrifice-three-lands-remove-defender"));
}
