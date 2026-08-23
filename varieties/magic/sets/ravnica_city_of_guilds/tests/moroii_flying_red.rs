//! Red coverage probe for Moroii's static Flying keyword.
//!
//! The separate upkeep regression covers its stack-backed life-loss trigger.

use cardbench_magic_engine::Keyword;
use cardbench_magic_rav::card_definitions;

#[test]
fn moroii_exposes_its_supported_flying_slice() {
    let moroii = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MOROII")
        .expect("Moroii definition exists");
    assert_eq!(moroii.keywords, [Keyword::Flying]);
    assert_eq!(
        moroii.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "upkeep-controller-life-loss",
        ]
    );
}
