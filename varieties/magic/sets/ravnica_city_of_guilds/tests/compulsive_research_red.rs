//! Red regression for Compulsive Research's private discard choice.

use cardbench_magic_engine::{CardType, Color, ManaCost, TargetRequirement};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn compulsive_research_requires_target_draw_and_conditional_private_discard_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-COMPULSIVE-RESEARCH")
        .expect("Compulsive Research definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Sorcery].into_iter().collect()
    );
    assert!(
        definition
            .supported_rules
            .contains(&"target-player-draw-three-conditional-private-discard")
    );
    assert_eq!(
        definition
            .effects
            .iter()
            .filter_map(cardbench_magic_engine::Effect::target_requirement)
            .collect::<Vec<_>>(),
        vec![TargetRequirement::Player],
        "the spell's recipient remains an ordinary player target"
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
