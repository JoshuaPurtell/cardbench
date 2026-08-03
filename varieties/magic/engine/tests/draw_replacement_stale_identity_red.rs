//! Red regression: a policy-submitted draw choice must not be replayable at a
//! later draw step for the same player.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, ManaCost, PlayerId, PolicyAction, Step, Zone,
};

const DRAW_CARD: &str = "TST-STALE-DRAW-CARD";

fn draw_card_definition() -> CardDefinition {
    CardDefinition {
        id: DRAW_CARD,
        name: DRAW_CARD,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["draw-replacement-stale-identity-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_round(game: &mut Game) {
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .attackers_declared
    {
        game.declare_attackers(game.next_policy_player(), &[])
            .expect("active player declares no attackers");
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .blockers_declared
    {
        game.declare_blockers(game.next_policy_player(), &[])
            .expect("active player declares no blockers");
    }
    for _ in 0..2 {
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary intervening draw resolves");
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("current priority holder passes");
    }
}

fn advance_to(game: &mut Game, target_turn: u32, target_step: Step) {
    for _ in 0..96 {
        if game.turn == target_turn && game.step == target_step {
            return;
        }
        pass_round(game);
    }
    panic!(
        "turn machine did not reach turn {target_turn} step {target_step:?}; reached turn {} step {:?}",
        game.turn, game.step
    );
}

#[test]
#[allow(clippy::too_many_lines)] // The trace crosses two complete turns to replay one exact stale policy action.
fn stale_draw_replacement_action_cannot_consume_a_later_draw_step() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new([draw_card_definition()], 2).expect("fixture builds");
    for player in [first, second] {
        for _ in 0..4 {
            game.add_card(player, DRAW_CARD, Zone::Library)
                .expect("draw margin begins in library");
        }
    }
    game.begin_game().expect("game begins");

    advance_to(&mut game, 2, Step::Draw);
    assert_eq!(game.active_player, second);
    assert!(
        game.view_for_player(second)
            .expect("first draw view")
            .draw_replacement_pending
    );
    game.submit_policy_move(
        second,
        "draw-replacement-stale.first-normal-draw.v1",
        PolicyAction::Draw { dredge: None },
    )
    .expect("first normal draw resolves");
    let first_hand_size = game
        .player(second)
        .expect("second player exists")
        .hand
        .len();
    assert_eq!(first_hand_size, 1);

    advance_to(&mut game, 4, Step::Draw);
    assert_eq!(game.active_player, second);
    assert!(
        game.view_for_player(second)
            .expect("second draw view")
            .draw_replacement_pending
    );
    let stale_result = game.submit_policy_move(
        second,
        "draw-replacement-stale.replay.v1",
        PolicyAction::Draw { dredge: None },
    );

    eprintln!(
        "draw-replacement stale-choice red trace: first_hand_size={first_hand_size}; turn={}; step={:?}; result={stale_result:?}; hand={:?}; events={:?}",
        game.turn,
        game.step,
        game.player(second).expect("second player exists").hand,
        game.canonical_event_log(),
    );
    assert!(
        stale_result.is_err(),
        "a normal-draw action from turn two must not consume the turn-four draw boundary"
    );
}
