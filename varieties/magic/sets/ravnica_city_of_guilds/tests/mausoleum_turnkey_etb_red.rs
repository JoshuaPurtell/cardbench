//! Red regression for Mausoleum Turnkey's conditional graveyard-return ETB.

use cardbench_magic_rav::{card_definitions, rav_triggered_ability_bindings};

#[test]
fn mausoleum_turnkey_declares_its_conditional_graveyard_return_slice() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MAUSOLEUM-TURNKEY")
        .expect("Mausoleum Turnkey definition exists");
    assert!(
        definition
            .supported_rules
            .contains(&"enter-battlefield-conditional-graveyard-return-to-hand")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-MAUSOLEUM-TURNKEY"
            && binding.ability.id == "conditional-graveyard-return"
    }));
}
