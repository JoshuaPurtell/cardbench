//! Red regression: setup-only tap mutation must not rewrite a live game.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, ManaCost, PlayerId, RulesError, Zone,
};

const MOUNTAIN: &str = "SETUP-MUTATION-MOUNTAIN";

fn mountain() -> CardDefinition {
    CardDefinition {
        id: MOUNTAIN,
        name: "Setup Mutation Mountain",
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
fn setup_tap_hook_rejects_live_game_mutation_after_a_mana_ability() {
    let player = PlayerId(0);
    let mut game = Game::new(vec![mountain()], 2).expect("fixture initializes");
    let land = game
        .add_card(player, MOUNTAIN, Zone::Battlefield)
        .expect("land begins on battlefield");
    game.begin_game().expect("first turn reaches upkeep priority");
    game.activate_mana_ability(player, land, Color::Red)
        .expect("legal intrinsic activation taps the source");
    game.clear_event_log();

    let result = game.set_tapped_for_setup(land, false);

    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction("tap state is setup-only"))
        ),
        "live setup hook rewrote an activated mana source; result={result:?}; tapped={}; events={:?}",
        game.object(land).expect("land remains addressable").tapped,
        game.canonical_event_log(),
    );
    assert!(
        game.object(land).expect("land remains addressable").tapped,
        "the rejected live setup mutation must preserve the tap cost"
    );
    assert!(
        game.canonical_event_log().is_empty(),
        "the setup seam must not manufacture a live event"
    );
    game.validate_invariants()
        .expect("the rejected setup mutation preserves state-machine invariants");
}
