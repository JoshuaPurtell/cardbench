//! Red discovery contract for Mausoleum Turnkey's optional ETB return.

use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

#[test]
fn mausoleum_turnkey_requires_a_policy_submitted_optional_etb_return() {
    let turnkey = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MAUSOLEUM-TURNKEY")
        .expect("Mausoleum Turnkey definition exists");
    let ability = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == turnkey.id
                && binding.ability.id == "conditional-graveyard-return"
        })
        .expect("conditional graveyard-return trigger exists");

    assert!(
        ability.ability.optional,
        "the ETB return must wait for its controller's explicit may decision"
    );
    assert!(
        turnkey
            .supported_rules
            .contains(&"policy-submitted-optional-etb-target-selection"),
        "the definition must expose policy-submitted target and may choices"
    );
    assert!(
        !turnkey
            .supported_rules
            .contains(&"deterministic-etb-target-selection"),
        "the retired deterministic target shortcut must not remain declared"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&turnkey.id),
        "the policy-submitted target and optional return complete the card"
    );
}
