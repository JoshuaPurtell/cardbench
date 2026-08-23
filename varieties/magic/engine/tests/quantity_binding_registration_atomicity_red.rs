//! A rejected quantity-replacement binding batch must not partially configure a game.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Game, ManaCost, ReplacementEffect, ReplacementEffectBinding,
};

const DOUBLER: &str = "QUANTITY-REGISTRATION-ATOMIC-DOUBLER";

fn definitions() -> Vec<CardDefinition> {
    vec![CardDefinition {
        id: DOUBLER,
        name: DOUBLER,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["quantity-replacement"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }]
}

#[test]
fn rejected_quantity_replacement_binding_batch_leaves_no_partial_registration() {
    let binding = ReplacementEffectBinding {
        source_definition: DOUBLER,
        effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 2 },
    };
    let mut game = Game::new(definitions(), 2).expect("fixture initializes");

    assert!(
        game.register_replacement_effect_bindings([binding, binding])
            .is_err(),
        "the duplicate batch must reject"
    );
    assert!(
        game.event_log.is_empty(),
        "configuration failures must not fabricate gameplay receipts"
    );
    game.validate_invariants()
        .expect("the partial configuration is shape-valid, so retry proves rollback");

    game.register_replacement_effect_bindings([binding])
        .expect("a valid retry must not observe state from the rejected batch");
}
