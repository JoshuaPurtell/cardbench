//! Public direct-draw timing contract.
//!
//! The engine's only ordinary draw path is the active player's draw step. A
//! public helper must not let a caller move a library card to hand at an
//! arbitrary priority-bearing step without a draw instruction or policy
//! receipt.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Game, ManaCost, PlayerId, Step, Zone};

const FILLER: &str = "DIRECT-DRAW-FILLER";

fn game() -> Game {
    Game::new(
        vec![CardDefinition {
            id: FILLER,
            name: "Direct Draw Filler",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["fixture"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        }],
        2,
    )
    .expect("fixture game initializes")
}

#[test]
fn public_direct_draw_is_rejected_outside_the_draw_step() {
    let player = PlayerId(0);
    let mut game = game();
    let card = game
        .add_card(player, FILLER, Zone::Library)
        .expect("fixture card enters the library");
    game.begin_game().expect("game reaches upkeep");
    assert_eq!(game.step, Step::Upkeep);
    let before_events = game.canonical_event_log();

    let result = game.draw_card(player, None);

    assert!(
        result.is_err(),
        "a direct live-game draw succeeded outside the draw step; card zone: {:?}; new events: {:?}",
        game.zone_of(card),
        &game.canonical_event_log()[before_events.len()..],
    );
}
