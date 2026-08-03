//! Red regression for Cleanup's mandatory discard-to-hand-size action.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Game, GameEvent, ManaCost, PlayerId, Step, Zone,
};

const FILLER: &str = "CLEANUP-HAND-SIZE-FILLER";

fn definitions() -> Vec<CardDefinition> {
    vec![CardDefinition {
        id: FILLER,
        name: FILLER,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["test-filler"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }]
}

fn advance_to_second_upkeep(game: &mut Game) {
    for _ in 0..128 {
        if game.turn == 2 && game.step == Step::Upkeep {
            return;
        }
        if game.step == Step::DeclareAttackers
            && !game
                .view_for_player(game.next_policy_player())
                .expect("combat view")
                .attackers_declared
        {
            game.declare_attackers(game.next_policy_player(), &[])
                .expect("empty attackers are explicit");
        }
        if game.step == Step::DeclareBlockers
            && !game
                .view_for_player(game.next_policy_player())
                .expect("combat view")
                .blockers_declared
        {
            game.declare_blockers(game.next_policy_player(), &[])
                .expect("empty blockers are explicit");
        }
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("ordinary priority pass advances the turn");
    }
    panic!("fixture did not reach player one's second-turn upkeep");
}

#[test]
fn cleanup_discards_down_to_the_default_hand_size_before_next_turn() {
    let player = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("fixture initializes");
    for _ in 0..8 {
        game.add_card(player, FILLER, Zone::Hand)
            .expect("eight-card hand is legal setup");
    }

    game.begin_game().expect("fixture game begins");
    advance_to_second_upkeep(&mut game);

    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::StepBegan {
                turn: 1,
                active_player,
                step: Step::Cleanup,
            } if *active_player == player
        )
    }));
    assert_eq!(
        game.player(player).expect("player exists").hand.len(),
        7,
        "Cleanup must discard to the default seven-card hand size before the next turn"
    );
    game.validate_invariants()
        .expect("the completed turn transition remains invariant-valid");
}
