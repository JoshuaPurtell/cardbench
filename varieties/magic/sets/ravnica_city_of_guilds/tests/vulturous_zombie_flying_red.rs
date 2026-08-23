//! Static regression for Vulturous Zombie's Flying keyword.

use cardbench_magic_engine::Keyword;
use cardbench_magic_rav::card_definitions;

#[test]
fn vulturous_zombie_retains_flying_alongside_its_full_trigger_binding() {
    let zombie = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VULTUROUS-ZOMBIE")
        .expect("Vulturous Zombie definition exists");
    assert_eq!(zombie.keywords, [Keyword::Flying]);
    assert_eq!(
        zombie.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "opponent-card-to-graveyard-plus-one-counter",
        ]
    );
}
