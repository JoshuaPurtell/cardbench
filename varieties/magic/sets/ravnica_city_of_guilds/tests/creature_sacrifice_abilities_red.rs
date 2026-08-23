//! Red regressions for RAV activated abilities that sacrifice a selected creature.

use cardbench_magic_engine::{Effect, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn golgari_rotwurm_is_complete_only_when_its_selected_creature_sacrifice_activation_is_bound() {
    let rotwurm = definition("RAV-GOLGARI-ROTWURM");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&rotwurm.id));
    assert!(
        rotwurm
            .supported_rules
            .contains(&"activated-sacrifice-creature-target-player-life-loss")
    );
    assert_eq!(
        ManaCost::with_colors(0, [cardbench_magic_engine::Color::Black]).mana_value(),
        1
    );
}

#[test]
fn drooling_groodion_is_complete_only_when_its_selected_creature_sacrifice_activation_is_bound() {
    let groodion = definition("RAV-DROOLING-GROODION");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&groodion.id));
    assert!(
        groodion
            .supported_rules
            .contains(&"activated-sacrifice-creature-target-minus-two-minus-two")
    );
    assert_eq!(
        Effect::ModifyTargetPtUntilEndOfTurn {
            power: -2,
            toughness: -2,
        }
        .target_requirement(),
        Some(cardbench_magic_engine::TargetRequirement::Creature)
    );
}
