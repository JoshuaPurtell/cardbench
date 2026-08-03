//! Red-to-green contract for Mindmoil's cast-trigger hand recycle.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_triggered_ability_bindings,
};

#[test]
fn mindmoil_requires_exact_cast_trigger_hand_bottom_draw_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MINDMOIL")
        .expect("Mindmoil definition exists");

    assert_eq!(
        executable_definition_id_for_collector(135),
        Ok("RAV-MINDMOIL")
    );
    assert_eq!(definition.name, "Mindmoil");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Red, Color::Red])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"controller-casts-spell-private-hand-bottom-draw-same-count")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.ability.id == "controller-casts-spell-hand-bottom-draw-same-count"
    }));
}
