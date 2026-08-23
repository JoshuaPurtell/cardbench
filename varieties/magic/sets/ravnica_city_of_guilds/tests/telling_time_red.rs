//! Red regression for Telling Time's private top-library partition.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn telling_time_requires_a_private_top_library_partition_effect() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TELLING-TIME")
        .expect("Telling Time definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Instant].into_iter().collect()
    );
    assert!(
        definition
            .supported_rules
            .contains(&"private-top-library-hand-top-bottom-partition")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
