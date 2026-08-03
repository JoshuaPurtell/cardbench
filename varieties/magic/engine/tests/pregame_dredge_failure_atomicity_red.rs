//! Red regression: a rejected pregame Dredge selector must not consume an ID.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, DecisionId, Game, ManaCost, ObjectId, PlayerId, Step, Zone,
};

const FILLER: &str = "TST-PREGAME-DREDGE-FILLER";

fn game() -> Game {
    Game::new(
        [CardDefinition {
            id: FILLER,
            name: FILLER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["test-only-pregame-dredge-filler"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        }],
        2,
    )
    .expect("fixture initializes")
}

fn advance_to_second_players_draw(game: &mut Game) {
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
        "fixture did not reach player one's draw; reached turn {} {:?} for {:?}",
        game.turn, game.step, game.active_player
    );
}

#[test]
fn rejected_pregame_dredge_selector_does_not_consume_the_next_draw_decision_id() {
    let mut game = game();
    for player in [PlayerId(0), PlayerId(1)] {
        game.add_card(player, FILLER, Zone::Library)
            .expect("library draw margin exists");
    }

    let rejected = game.draw_card(PlayerId(0), Some(ObjectId(999)));
    eprintln!("rejected pregame dredge selector: {rejected:?}");
    assert!(rejected.is_err(), "an unknown Dredge source must reject");

    game.begin_game().expect("fixture begins after the rejected helper");
    advance_to_second_players_draw(&mut game);
    let draw_view = game
        .view_for_player(PlayerId(1))
        .expect("draw view is available");
    let decision = draw_view
        .draw_replacement_decision
        .expect("real draw exposes its public decision id");
    assert_eq!(
        decision,
        DecisionId(1),
        "a rejected pregame compatibility helper must not silently consume a future public decision id"
    );
    game.validate_invariants()
        .expect("the real draw boundary remains invariant-valid");
}
