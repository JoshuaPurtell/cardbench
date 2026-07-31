//! Red coverage probe for Undercity Shade's black-only blocking restriction.
//!
//! Its activated power/toughness ability remains intentionally outside this
//! static evasion probe and must not be approximated.

use cardbench_magic_rav::card_definitions;

#[test]
fn undercity_shade_exposes_its_supported_black_only_evasion_slice() {
    let shade = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-UNDERCITY-SHADE")
        .expect("Undercity Shade definition exists");
    assert_eq!(
        shade
            .keywords
            .iter()
            .map(|keyword| format!("{keyword:?}"))
            .collect::<Vec<_>>(),
        ["BlackEvasion"]
    );
    assert_eq!(
        shade.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "black-only-evasion"
        ]
    );
}
