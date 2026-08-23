//! Red regression for Bloodletter Quill's two independent paid abilities.
//!
//! The green milestone must keep counter placement, the post-draw dynamic
//! life loss, and the blue-black counter-removal cost in expansion-neutral
//! engine data.  A card-name dispatch or a fixed one-life approximation is
//! not sufficient.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn bloodletter_quill_declares_both_counter_abilities_at_full_fidelity() {
    let quill = definition("RAV-BLOODLETTER-QUILL");
    assert_eq!(quill.mana_cost, ManaCost::new(3));
    assert_eq!(quill.card_types, [CardType::Artifact].into_iter().collect());
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&quill.id),
        "Bloodletter Quill is full only when both printed activations are represented"
    );

    let abilities = rav_activated_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == quill.id)
        .map(|binding| binding.ability)
        .collect::<Vec<_>>();
    assert_eq!(abilities.len(), 2, "Quill has exactly two activations");

    let draw = abilities
        .iter()
        .find(|ability| ability.id == "two-tap-add-blood-draw-lose-for-blood")
        .expect("paid draw ability exists");
    assert_eq!(draw.mana_cost, ManaCost::new(2));
    assert!(draw.tap_cost);

    let remove = abilities
        .iter()
        .find(|ability| ability.id == "blue-black-remove-blood")
        .expect("blue-black blood removal ability exists");
    assert_eq!(
        remove.mana_cost,
        ManaCost::with_colors(0, [Color::Blue, Color::Black])
    );
    assert!(!remove.tap_cost);
}
