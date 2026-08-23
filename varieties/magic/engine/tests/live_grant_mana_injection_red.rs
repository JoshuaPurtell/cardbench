//! Red regression: setup mana injection must stop at the live-game boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Game, ManaCost, PlayerId};

const SPELL: &str = "LIVE-GRANT-MANA-INJECTION";

fn spell() -> CardDefinition {
    CardDefinition {
        id: SPELL,
        name: SPELL,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["live-setup-boundary-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn grant_mana_cannot_inject_floating_mana_after_game_start() {
    let player = PlayerId(0);
    let mut game = Game::new([spell()], 2).expect("fixture game initializes");
    game.begin_game().expect("fixture reaches live priority");
    let events_before = game.canonical_event_log();
    let mana_before = game
        .player(player)
        .expect("player exists")
        .mana_pool
        .clone();

    let result = game.grant_mana(player, Color::Blue, 1);
    eprintln!(
        "live grant-mana result: {result:?}; mana={:?}; events={:?}",
        game.player(player).expect("player exists").mana_pool,
        game.canonical_event_log(),
    );

    assert!(
        result.is_err(),
        "live setup mana injection must be rejected before changing the pool"
    );
    assert_eq!(
        game.player(player).expect("player exists").mana_pool,
        mana_before
    );
    assert_eq!(game.canonical_event_log(), events_before);
    game.validate_invariants()
        .expect("rejected live setup mana injection preserves invariants");
}
