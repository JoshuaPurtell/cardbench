//! Red regressions: rejected ability-cost setup batches retain no prefix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, ActivatedManaAbility, CardDefinition, CardType,
    Color, Effect, Game, GeneralizedActivatedAbilityCost, ManaAbilityBinding,
    ManaAbilityCostBinding, ManaAbilityOutput, ManaCost, RulesError,
};

const SOURCE: &str = "TST-ATOMIC-ABILITY-COST-SOURCE";
const ACTIVATED: &str = "test-activated";
const MANA: &str = "test-mana";

fn source_definition() -> CardDefinition {
    CardDefinition {
        id: SOURCE,
        name: SOURCE,
        set_code: "TST",
        mana_cost: ManaCost::new(2),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["test-only-ability-cost-source"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn activated_ability_binding() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: SOURCE,
        ability: ActivatedAbility {
            id: ACTIVATED,
            mana_cost: ManaCost::new(0),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 1 }],
        },
    }
}

fn mana_ability_binding() -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition: SOURCE,
        ability: ActivatedManaAbility {
            id: MANA,
            tap_cost: true,
            output: ManaAbilityOutput::Fixed(Color::Green),
            amount: 1,
            life_payment: None,
            controller_damage: None,
        },
    }
}

#[test]
fn rejected_generalized_activated_cost_batch_leaves_no_prefix_binding() {
    let mut game = Game::new_with_all_bindings(
        [source_definition()],
        2,
        [],
        [],
        [],
        [activated_ability_binding()],
    )
    .expect("fixture initializes");
    let binding = cardbench_magic_engine::ActivatedAbilityCostBinding {
        card_definition: SOURCE,
        ability_id: ACTIVATED,
        cost: GeneralizedActivatedAbilityCost {
            life_payment: 1,
            ..GeneralizedActivatedAbilityCost::default()
        },
    };

    let result = game
        .register_generalized_activated_ability_cost_bindings([binding.clone(), binding.clone()]);
    eprintln!("rejected generalized-ability-cost batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate generalized activated-cost binding"
        ))
    ));
    game.register_generalized_activated_ability_cost_bindings([binding])
        .expect("a rejected batch must leave no generalized-cost prefix");
    game.validate_invariants().expect("repaired setup is valid");
}

#[test]
fn rejected_mana_ability_cost_batch_leaves_no_prefix_binding() {
    let mut game =
        Game::new_with_mana_abilities([source_definition()], 2, [mana_ability_binding()])
            .expect("fixture initializes");
    let binding = ManaAbilityCostBinding {
        card_definition: SOURCE,
        ability_id: MANA,
        sacrifice_source: true,
    };

    let result = game.register_mana_ability_cost_bindings([binding, binding]);
    eprintln!("rejected mana-ability-cost batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate mana ability cost binding"
        ))
    ));
    game.register_mana_ability_cost_bindings([binding])
        .expect("a rejected batch must leave no mana-cost prefix");
    game.validate_invariants().expect("repaired setup is valid");
}
