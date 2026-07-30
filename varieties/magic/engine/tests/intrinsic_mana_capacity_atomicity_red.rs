//! Red regression: intrinsic mana abilities must not log saturated output.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, ManaCost, PlayerId, RulesError,
};

const MOUNTAIN: &str = "TEST-MOUNTAIN";

fn mountain() -> CardDefinition {
    CardDefinition {
        id: MOUNTAIN,
        name: "Test Mountain",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Red]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: true,
        supported_rules: &["intrinsic-mana"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn saturated_intrinsic_mana_ability_rejects_before_tapping_or_logging_output() {
    let player = PlayerId(0);
    let mut game = Game::new(vec![mountain()], 2).expect("fixture initializes");
    let land = game
        .put_on_battlefield(player, MOUNTAIN)
        .expect("land begins on the battlefield");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.grant_mana(player, Color::Red, u8::MAX)
        .expect("the bounded pool accepts its maximum representable amount");
    game.clear_event_log();

    let result = game.activate_mana_ability(player, land, Color::Red);

    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction(
                "mana pool cannot hold the requested mana"
            ))
        ),
        "a full pool accepted a land activation with a phantom receipt; tapped={}, events={:?}",
        game.object(land).expect("land remains addressable").tapped,
        game.event_log,
    );
    assert!(
        !game.object(land).expect("land remains addressable").tapped,
        "a rejected ability must not tap its source"
    );
    assert!(
        game.event_log.is_empty(),
        "a rejected ability must not log phantom mana"
    );
    game.validate_invariants()
        .expect("the rejection remains state-machine valid");
}
