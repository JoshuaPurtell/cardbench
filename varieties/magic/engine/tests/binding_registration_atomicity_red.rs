//! A rejected expansion binding batch must not partially configure a game.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, DamageReplacementEffect, DamageReplacementEffectBinding, Game,
    ManaCost,
};

const HALVER: &str = "REGISTRATION-ATOMIC-HALVER";

fn definitions() -> Vec<CardDefinition> {
    vec![CardDefinition {
        id: HALVER,
        name: HALVER,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["damage-replacement"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }]
}

#[test]
fn rejected_damage_replacement_binding_batch_leaves_no_partial_registration() {
    let binding = DamageReplacementEffectBinding {
        source_definition: HALVER,
        effect: DamageReplacementEffect::HalveDamage,
    };
    let mut game = Game::new(definitions(), 2).expect("fixture initializes");

    let rejected = game.register_damage_replacement_effect_bindings([binding, binding]);
    assert!(
        rejected.is_err(),
        "fixture batch must be rejected: {rejected:?}"
    );
    assert!(
        game.event_log.is_empty(),
        "configuration failures must not fabricate gameplay receipts"
    );
    game.validate_invariants()
        .expect("the partial configuration is shape-valid, so rollback needs a behavioral probe");

    game.register_damage_replacement_effect_bindings([binding])
        .expect("a valid retry must not observe state from the rejected batch");
}
