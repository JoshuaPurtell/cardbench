//! Red regression for Cleanup's mandatory discard-to-hand-size action.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, DecisionKind, DecisionSelection, DecisionVisibility, Game, GameEvent,
    ManaCost, ObjectId, PlayerId, Step, Zone,
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

fn advance_to_cleanup_discard(game: &mut Game, player: PlayerId) -> (u64, ObjectId) {
    for _ in 0..128 {
        let view = game.view_for_player(player).expect("active-player view");
        if let Some(decision) = view.pending_decision {
            assert_eq!(decision.kind, DecisionKind::CleanupDiscard);
            assert_eq!(decision.visibility, DecisionVisibility::Private);
            assert_eq!(decision.min_selections, 1);
            assert_eq!(decision.max_selections, 1);
            let selected = decision
                .candidates
                .first()
                .expect("over-limit hand exposes a discard candidate")
                .id;
            return (decision.id.0, selected);
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
    panic!("fixture did not reach Cleanup's mandatory discard decision");
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
    let (decision, discarded) = advance_to_cleanup_discard(&mut game, player);
    let opponent = PlayerId(1);
    assert!(
        game.view_for_player(opponent)
            .expect("opponent view")
            .pending_decision
            .is_none(),
        "Cleanup candidates remain private to the active player"
    );
    let events_before_wrong_player = game.event_log.len();
    assert!(
        game.submit_decision(
            opponent,
            cardbench_magic_engine::DecisionId(decision),
            DecisionSelection::Objects(vec![discarded]),
        )
        .is_err(),
        "only the active player may answer Cleanup's private decision"
    );
    assert_eq!(game.event_log.len(), events_before_wrong_player);
    game.validate_invariants()
        .expect("the private Cleanup decision remains invariant-valid");

    game.submit_decision(
        player,
        cardbench_magic_engine::DecisionId(decision),
        DecisionSelection::Objects(vec![discarded]),
    )
    .expect("the active player discards exactly the excess card");

    assert_eq!(game.turn, 2);
    assert_eq!(game.step, Step::Upkeep);

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
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert!(game.event_log.windows(4).any(|events| {
        matches!(
            events,
            [
                GameEvent::DecisionOpened {
                    decision: opened,
                    kind: DecisionKind::CleanupDiscard,
                    visibility: DecisionVisibility::Private,
                    ..
                },
                GameEvent::DecisionCompleted {
                    decision: completed,
                    kind: DecisionKind::CleanupDiscard,
                    ..
                },
                GameEvent::CardDiscarded {
                    player: discarded_player,
                    card,
                },
                GameEvent::CardMoved {
                    card: moved,
                    to: Zone::Graveyard,
                    ..
                },
            ] if opened.0 == decision && completed.0 == decision && *discarded_player == player && *card == discarded && *moved == discarded
        )
    }));
    game.validate_invariants()
        .expect("the completed turn transition remains invariant-valid");
}
