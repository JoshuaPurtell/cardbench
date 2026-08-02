//! Red regression: a target-player color choice must be able to suspend one
//! instruction in an otherwise ordinary activated-ability stack suffix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Game,
    ManaCost, TargetRequirement,
};

const SOURCE: &str = "TST-MULTI-INSTRUCTION-TARGET-PLAYER-MANA";

fn source_definition() -> CardDefinition {
    CardDefinition {
        id: SOURCE,
        name: SOURCE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["multi-instruction-target-player-mana-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn target_player_mana_choice_can_appear_in_a_middle_activated_ability_instruction() {
    let result = Game::new_with_all_bindings(
        [source_definition()],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: "gain-choose-gain",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Player],
                effects: vec![
                    Effect::GainLifeController { amount: 1 },
                    Effect::AddOneManaOfTargetPlayersChosenColor,
                    Effect::GainLifeController { amount: 2 },
                ],
            },
        }],
    );

    eprintln!("multi-instruction target-player-mana red construction: {result:?}");
    assert!(
        result.is_ok(),
        "a target-player mana choice is a normal resolution instruction, not a whole-ability restriction"
    );
}
