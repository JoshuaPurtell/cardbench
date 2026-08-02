//! RED: attachment support must not be limited to Aura spells.
//!
//! An Equipment's ordinary attach ability is a stack-using activated ability.
//! Before the general attachment substrate, the engine rejected its typed
//! attachment effect during immutable binding validation, so no generic
//! Equipment could enter the game at all.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, ContinuousChange,
    Effect, Game, ManaCost, TargetRequirement,
};

const EQUIPMENT: &str = "TST-GENERAL-ATTACHMENT-EQUIPMENT";

fn definition(id: &'static str, card_type: CardType) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["general-attachment-red-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn equipment_attach_ability_is_a_valid_generic_activated_ability() {
    let game = Game::new_with_all_bindings(
        vec![definition(EQUIPMENT, CardType::Artifact)],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: EQUIPMENT,
            ability: ActivatedAbility {
                id: "attach",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: true,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![ContinuousChange::ModifyPowerToughness {
                        power: 1,
                        toughness: 1,
                    }],
                }],
            },
        }],
    );

    assert!(
        game.is_ok(),
        "a typed Equipment attachment ability must be constructible: {game:?}"
    );
}
