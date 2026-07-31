//! Red coverage probe for Vulturous Zombie's static Flying keyword.
//!
//! Its graveyard-triggered counter ability remains intentionally outside this
//! static keyword probe and must not be approximated.

use cardbench_magic_engine::Keyword;
use cardbench_magic_rav::card_definitions;

#[test]
fn vulturous_zombie_exposes_its_supported_flying_slice() {
    let zombie = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VULTUROUS-ZOMBIE")
        .expect("Vulturous Zombie definition exists");
    assert_eq!(zombie.keywords, [Keyword::Flying]);
    assert_eq!(
        zombie.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
}
