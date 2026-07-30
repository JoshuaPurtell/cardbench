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

fn advance_unstarted_fixture_to_second_players_draw(game: &mut Game) {
    for _ in 0..64 {
        if game.turn == 2 && game.active_player == PlayerId(1) && game.step == Step::Draw {
            return;
        }
        if game.step == Step::DeclareAttackers
            && !game
                .view_for_player(game.next_policy_player())
                .expect("combat view")
                .attackers_declared
        {
            game.declare_attackers(game.next_policy_player(), &[])
                .expect("empty attack declaration advances the fixture");
            continue;
        }
        if game.step == Step::DeclareBlockers
            && !game
                .view_for_player(game.next_policy_player())
                .expect("combat view")
                .blockers_declared
        {
            game.declare_blockers(game.next_policy_player(), &[])
                .expect("empty block declaration advances the fixture");
            continue;
        }
        game.pass_priority(game.priority)
            .expect("fixture priority pass advances the turn state");
    }
    panic!(
        "unstarted fixture did not reach player one's draw; reached turn {} {:?} for {:?}",
        game.turn, game.step, game.active_player
    );
}

#[test]
fn pending_draw_resolution_clears_its_marker_in_an_unstarted_fixture_turn() {
    let player = PlayerId(1);
    let mut game = game();
    let drawn = game
        .add_card(player, FILLER, Zone::Library)
        .expect("fixture supplies player one's draw");
    advance_unstarted_fixture_to_second_players_draw(&mut game);
    let pending_view = game
        .view_for_player(player)
        .expect("pending draw view is available");
    assert!(pending_view.draw_replacement_pending);

    game.resolve_pending_draw(player, None)
        .expect("the fixture's ordinary draw resolves");
    let resolved_view = game
        .view_for_player(player)
        .expect("resolved draw view is available");
    assert!(
        !resolved_view.draw_replacement_pending,
        "a resolved ordinary draw must clear its marker even before begin_game; card zone: {:?}; events: {:?}; view: {resolved_view:?}",
        game.zone_of(drawn),
        game.event_log
    );
    game.pass_priority(player)
        .expect("the resolved fixture draw restores its ordinary priority window");
    game.validate_invariants()
        .expect("the fixture transition remains internally valid");
}
