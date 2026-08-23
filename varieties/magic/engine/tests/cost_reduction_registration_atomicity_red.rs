//! Red regression: rejected setup binding batches must not retain a prefix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CostReductionBinding, Game, ManaCost, RulesError,
};

const REDUCER: &str = "TST-ATOMIC-REDUCER";

fn reducer_definition() -> CardDefinition {
    CardDefinition {
        id: REDUCER,
        name: REDUCER,
        set_code: "TST",
        mana_cost: ManaCost::new(2),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["test-only-reducer"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn binding() -> CostReductionBinding {
    CostReductionBinding {
        source_definition: REDUCER,
        generic_amount: 1,
        noncreature_only: true,
    }
}

#[test]
fn rejected_cost_reduction_batch_leaves_no_prefix_binding() {
    let mut game = Game::new([reducer_definition()], 2).expect("fixture initializes");

    let result = game.register_cost_reduction_bindings([binding(), binding()]);
    eprintln!("rejected reduction batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate cost-reduction binding for card definition"
        ))
    ));

    game.register_cost_reduction_bindings([binding()])
        .expect("a rejected batch must leave setup available for an exact retry");
    game.validate_invariants()
        .expect("the repaired registration leaves a valid setup state");
}
