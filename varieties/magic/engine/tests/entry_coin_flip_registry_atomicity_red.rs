//! Red regression: rejected entry coin-flip setup batches retain no prefix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, EntryCharacteristicOverride, EntryCoinFlipBinding, Game, ManaCost,
    RulesError,
};

const SOURCE: &str = "TST-ATOMIC-ENTRY-COIN-FLIP-SOURCE";

fn source_definition() -> CardDefinition {
    CardDefinition {
        id: SOURCE,
        name: SOURCE,
        set_code: "TST",
        mana_cost: ManaCost::new(2),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["test-only-entry-coin-flip-source"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn binding() -> EntryCoinFlipBinding {
    EntryCoinFlipBinding {
        card_definition: SOURCE,
        heads: EntryCharacteristicOverride {
            power: 3,
            toughness: 1,
            keywords: vec![],
        },
        tails: EntryCharacteristicOverride {
            power: 1,
            toughness: 3,
            keywords: vec![],
        },
    }
}

#[test]
fn rejected_entry_coin_flip_batch_leaves_no_prefix_binding() {
    let mut game = Game::new([source_definition()], 2).expect("fixture initializes");
    let binding = binding();

    let result = game.register_entry_coin_flip_bindings([binding.clone(), binding.clone()]);
    eprintln!("rejected entry-coin-flip batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate entry coin-flip binding"
        ))
    ));
    game.register_entry_coin_flip_bindings([binding])
        .expect("a rejected batch must leave no entry-coin-flip prefix");
    game.validate_invariants().expect("repaired setup is valid");
}
