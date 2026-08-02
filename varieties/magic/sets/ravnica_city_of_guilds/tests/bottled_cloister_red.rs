//! Red discovery contract for Bottled Cloister's linked hand-exile lifecycle.

use cardbench_magic_engine::{CardType, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn bottled_cloister_requires_opponent_upkeep_hand_exile_and_controller_return_draw() {
    let cloister = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BOTTLED-CLOISTER")
        .expect("Bottled Cloister definition exists");

    assert_eq!(cloister.name, "Bottled Cloister");
    assert_eq!(cloister.mana_cost, ManaCost::new(4));
    assert_eq!(
        cloister.card_types,
        [CardType::Artifact].into_iter().collect()
    );
    assert!(cloister.colors.is_empty());
    assert!(cloister.mana_colors.is_empty());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&cloister.id));
    assert!(cloister
        .supported_rules
        .contains(&"opponent-upkeep-linked-hand-exile"));
    assert!(cloister
        .supported_rules
        .contains(&"controller-upkeep-linked-hand-return-then-draw"));
}
