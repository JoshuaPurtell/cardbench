//! Red regression for Remand's any-spell counter and draw sequence.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn remand_requires_an_any_spell_counter_that_draws_its_controller() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-REMAND")
        .expect("Remand definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Instant].into_iter().collect()
    );
    assert!(definition
        .supported_rules
        .contains(&"counter-target-spell-then-draw-controller"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
