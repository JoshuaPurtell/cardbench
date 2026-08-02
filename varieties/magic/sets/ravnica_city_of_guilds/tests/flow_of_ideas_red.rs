//! Red regression for Flow of Ideas' typed basic-land draw count.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn flow_of_ideas_requires_a_controller_island_draw_effect() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FLOW-OF-IDEAS")
        .expect("Flow of Ideas definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(5, [Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Sorcery].into_iter().collect()
    );
    assert!(
        definition
            .supported_rules
            .contains(&"draw-for-each-controlled-island")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
