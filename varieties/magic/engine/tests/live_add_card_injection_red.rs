//! Red regression: setup card injection must stop at the live-game boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Game, ManaCost, PlayerId, Zone};

const SPELL: &str = "LIVE-ADD-CARD-INJECTION";

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
fn add_card_cannot_inject_a_new_object_after_game_start() {
    let player = PlayerId(0);
    let mut game = Game::new([spell()], 2).expect("fixture game initializes");
    game.begin_game().expect("fixture reaches live priority");
    let events_before = game.canonical_event_log();
    let hand_before = game.player(player).expect("player exists").hand.clone();

    let result = game.add_card(player, SPELL, Zone::Hand);
    eprintln!(
        "live add-card result: {result:?}; hand={:?}; events={:?}",
        game.player(player).expect("player exists").hand,
        game.canonical_event_log(),
    );

    assert!(
        result.is_err(),
        "live setup injection must be rejected before creating an object"
    );
    assert_eq!(
        &game.player(player).expect("player exists").hand,
        &hand_before
    );
    assert_eq!(game.canonical_event_log(), events_before);
    game.validate_invariants()
        .expect("rejected live setup injection preserves invariants");
}
