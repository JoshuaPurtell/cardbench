//! Red coverage probe for Moroii's static Flying keyword.
//!
//! Its upkeep life-loss trigger remains intentionally outside this static
//! keyword probe and must not be approximated.

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
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
}
