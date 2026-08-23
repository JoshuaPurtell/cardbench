//! Red regression: rejected activated-cost modifier batches must not retain a prefix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbilityCostModifier, ActivatedAbilityCostModifierBinding, CardDefinition, CardType,
    Game, ManaCost, RulesError,
};

const TAXER: &str = "TST-ATOMIC-ACTIVATION-TAXER";

fn taxer_definition() -> CardDefinition {
    CardDefinition {
        id: TAXER,
        name: TAXER,
        set_code: "TST",
        mana_cost: ManaCost::new(2),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["test-only-activation-taxer"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn binding() -> ActivatedAbilityCostModifierBinding {
    ActivatedAbilityCostModifierBinding {
        source_definition: TAXER,
        modifier: ActivatedAbilityCostModifier::IncreaseGeneric {
            amount: 1,
            nonmana_only: true,
        },
    }
}

#[test]
fn rejected_activated_cost_modifier_batch_leaves_no_prefix_binding() {
    let mut game = Game::new([taxer_definition()], 2).expect("fixture initializes");

    let result = game.register_activated_ability_cost_modifier_bindings([binding(), binding()]);
    eprintln!("rejected activated-cost-modifier batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate activated-cost modifier binding"
        ))
    ));

    game.register_activated_ability_cost_modifier_bindings([binding()])
        .expect("a rejected batch must leave setup available for an exact retry");
    game.validate_invariants()
        .expect("the repaired registration leaves a valid setup state");
}
