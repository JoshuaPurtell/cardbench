//! Red regression: rejected static attack-restriction batches retain no prefix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Game, ManaCost, RulesError, StaticAttackRestriction,
    StaticAttackRestrictionBinding,
};

const PEACEKEEPER: &str = "TST-ATOMIC-PEACEKEEPER";

fn peacekeeper_definition() -> CardDefinition {
    CardDefinition {
        id: PEACEKEEPER,
        name: PEACEKEEPER,
        set_code: "TST",
        mana_cost: ManaCost::new(2),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["test-only-peacekeeper"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn binding() -> StaticAttackRestrictionBinding {
    StaticAttackRestrictionBinding {
        card_definition: PEACEKEEPER,
        restriction: StaticAttackRestriction::OpponentsCannotAttackController,
    }
}

#[test]
fn rejected_static_attack_restriction_batch_leaves_no_prefix_binding() {
    let mut game = Game::new([peacekeeper_definition()], 2).expect("fixture initializes");

    let result = game.register_static_attack_restrictions([binding(), binding()]);
    eprintln!("rejected static-attack-restriction batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate static attack-restriction binding"
        ))
    ));

    game.register_static_attack_restrictions([binding()])
        .expect("a rejected batch must leave setup available for an exact retry");
    game.validate_invariants()
        .expect("the repaired registration leaves a valid setup state");
}
