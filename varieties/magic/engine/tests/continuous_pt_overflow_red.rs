//! Red regression: continuous-layer arithmetic must not panic mid-transition.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use cardbench_magic_engine::{
    CardDefinition, CardType, ContinuousChange, Duration, Game, ManaCost, PlayerId,
};

const CREATURE: &str = "TST-ONE-POWER-CREATURE";

fn creature() -> CardDefinition {
    CardDefinition {
        id: CREATURE,
        name: "one power creature",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn installing_a_representable_large_pt_modifier_does_not_panic_after_writing_its_receipt() {
    let player = PlayerId(0);
    let mut game = Game::new([creature()], 2).expect("two-player game initializes");
    let target = game
        .put_on_battlefield(player, CREATURE)
        .expect("creature starts on the battlefield");
    game.clear_event_log();

    let result = catch_unwind(AssertUnwindSafe(|| {
        game.add_continuous_effect(
            target,
            target,
            ContinuousChange::ModifyPowerToughness {
                power: i16::MAX,
                toughness: 0,
            },
            Duration::Permanent,
        )
    }));

    assert!(
        result.is_ok(),
        "continuous-effect installation panicked after a partial lifecycle transition; events: {:?}; effects: {:?}",
        game.canonical_event_log(),
        game.continuous_effects,
    );
    result
        .expect("installation must not panic")
        .expect("the layer transition produces an engine-defined result");
}
