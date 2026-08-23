//! Event and privacy contracts for Telling Time's top-library partition.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, DecisionVisibility, Game, GameEvent,
    PlayerId, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn game() -> Game {
    Game::new(card_definitions(), 2).expect("RAV game builds")
}

fn public_transition_snapshot(
    game: &Game,
) -> (
    Vec<cardbench_magic_engine::PlayerState>,
    Vec<cardbench_magic_engine::StackObject>,
    Vec<String>,
    PlayerId,
) {
    (
        game.players.clone(),
        game.stack.clone(),
        game.canonical_event_log(),
        game.priority,
    )
}

fn move_to_first_main(game: &mut Game) {
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
}

fn cast_telling_time(
    game: &mut Game,
) -> (
    cardbench_magic_engine::ObjectId,
    [cardbench_magic_engine::ObjectId; 3],
) {
    let telling_time = game
        .add_card(PlayerId(0), "RAV-TELLING-TIME", Zone::Hand)
        .expect("spell setup");
    let bottom = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("bottom setup");
    let middle = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("middle setup");
    let top = game
        .add_card(PlayerId(0), "RAV-SWAMP", Zone::Library)
        .expect("top setup");
    let islands = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("blue source")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    move_to_first_main(game);
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("blue payment");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: telling_time,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Telling Time casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    (telling_time, [top, middle, bottom])
}

#[test]
fn telling_time_partitions_private_snapshot_and_keeps_candidates_out_of_public_log() {
    let mut game = game();
    let (telling_time, [top, middle, bottom]) = cast_telling_time(&mut game);
    let controller = game
        .view_for_player(PlayerId(0))
        .expect("controller receives view");
    let decision = controller.pending_decision.expect("private decision opens");
    assert_eq!(decision.kind, DecisionKind::LibraryTopPartition);
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!(decision.min_selections, 3);
    assert_eq!(decision.max_selections, 3);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![top, middle, bottom],
        "only the controller sees the top-to-bottom snapshot"
    );
    let opponent = game
        .view_for_player(PlayerId(1))
        .expect("opponent receives view");
    assert!(opponent.pending_decision.is_none());
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardsLookedAt { .. } | GameEvent::CardRevealed { .. }
    )));

    let before_state = public_transition_snapshot(&game);
    assert!(
        game.submit_decision(
            PlayerId(1),
            decision.id,
            DecisionSelection::LibraryTopPartition {
                hand: top,
                top: Some(middle),
                bottom: vec![bottom],
            },
        )
        .is_err()
    );
    assert_eq!(
        public_transition_snapshot(&game),
        before_state,
        "foreign policy cannot mutate a private decision"
    );

    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::LibraryTopPartition {
            hand: top,
            top: Some(middle),
            bottom: vec![bottom],
        },
    )
    .expect("controller submits exhaustive partition");
    println!("telling_time_event_log={:#?}", game.canonical_event_log());
    assert_eq!(game.zone_of(top), Some(Zone::Hand));
    assert_eq!(game.players[PlayerId(0).0].library, vec![bottom, middle]);
    assert_eq!(game.zone_of(telling_time), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PrivateLibraryTopPartitionResolved { player, source, inspected: 3 }
            if *player == PlayerId(0) && *source == telling_time
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellResolved { card } if *card == telling_time
    )));
    game.validate_invariants()
        .expect("private partition trace preserves state-machine invariants");
}

#[test]
fn telling_time_short_library_requires_no_nonexistent_top_and_stale_decision_fails_atomically() {
    let mut game = game();
    let telling_time = game
        .add_card(PlayerId(0), "RAV-TELLING-TIME", Zone::Hand)
        .expect("spell setup");
    let only = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("only library card");
    let islands = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("blue source")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    move_to_first_main(&mut game);
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("blue payment");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: telling_time,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Telling Time casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("one-card decision opens");
    assert_eq!(decision.candidates.len(), 1);
    let before = public_transition_snapshot(&game);
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::LibraryTopPartition {
                hand: only,
                top: Some(only),
                bottom: vec![],
            },
        )
        .is_err()
    );
    assert_eq!(
        public_transition_snapshot(&game),
        before,
        "invalid short-library partition is atomic"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::LibraryTopPartition {
            hand: only,
            top: None,
            bottom: vec![],
        },
    )
    .expect("one card goes to hand");
    let after_resolution = public_transition_snapshot(&game);
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::LibraryTopPartition {
                hand: only,
                top: None,
                bottom: vec![],
            },
        )
        .is_err()
    );
    assert_eq!(
        public_transition_snapshot(&game),
        after_resolution,
        "stale decision id cannot mutate later state"
    );
    assert_eq!(game.zone_of(only), Some(Zone::Hand));
    game.validate_invariants()
        .expect("short-library partition preserves invariants");
}

#[test]
fn telling_time_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-TELLING-TIME"));
}
