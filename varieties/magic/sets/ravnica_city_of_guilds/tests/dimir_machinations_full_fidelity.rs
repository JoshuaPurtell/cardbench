//! Full-fidelity and event-log contract for Dimir Machinations.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, DecisionVisibility, Game, GameEvent,
    PlayerId, PolicyAction, PolicyMoveKind, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn move_to_first_main(game: &mut Game) {
    for _ in 0..2 {
        pass_pair(game);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The complete private target-library transcript is the card contract.
fn dimir_machinations_privately_reorders_a_target_players_top_three() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let machinations = game
        .add_card(PlayerId(0), "RAV-DIMIR-MACHINATIONS", Zone::Hand)
        .expect("Dimir Machinations enters hand");
    let bottom = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("bottom card enters target library");
    let first = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("first target library card");
    let second = game
        .add_card(PlayerId(1), "RAV-SWAMP", Zone::Library)
        .expect("second target library card");
    let third = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
        .expect("top target library card");
    let swamps = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-SWAMP")
                .expect("black source enters for setup")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    move_to_first_main(&mut game);
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("black mana source activates");
    }

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: machinations,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("black sorcery casts at sorcery speed");
    pass_pair(&mut game);

    let controller = game.view_for_player(PlayerId(0)).expect("controller view");
    let decision = controller
        .pending_decision
        .expect("the resolving spell opens one private decision");
    assert_eq!(decision.kind, DecisionKind::TargetPlayerLibraryTopReorder);
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![third, second, first],
        "only the controller sees the target library snapshot in top-first order"
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("target view")
            .pending_decision
            .is_none(),
        "the target player receives no candidate identity projection"
    );
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardRevealed { card, .. }
            if [first, second, third].contains(card)
    )));

    game.submit_policy_move(
        PlayerId(0),
        "rav-dimir-machinations.private-target-library-reorder.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::TargetPlayerLibraryTopReorder {
                top: vec![second],
                bottom: vec![first, third],
            },
        },
    )
    .expect("controller policy privately submits the complete top/bottom partition");

    println!(
        "Dimir Machinations event log: {:?}",
        game.canonical_event_log()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-DIMIR-MACHINATIONS"));
    assert_eq!(
        game.players[PlayerId(1).0].library,
        vec![first, third, bottom, second],
        "submitted bottom order is bottom-to-top and retained top order is top-to-bottom"
    );
    assert_eq!(game.zone_of(machinations), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PrivateTargetPlayerLibraryReorderOpened {
            decision: opened,
            controller,
            source,
            target,
            count: 3,
            ..
        } if *opened == decision.id
            && *controller == PlayerId(0)
            && *source == machinations
            && *target == PlayerId(1)
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PrivateTargetPlayerLibraryReordered {
            controller,
            source,
            target,
            inspected: 3,
            ..
        } if *controller == PlayerId(0)
            && *source == machinations
            && *target == PlayerId(1)
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PolicyMoveSubmitted { player, policy, kind: PolicyMoveKind::SubmitDecision }
            if *player == PlayerId(0)
                && policy == "rav-dimir-machinations.private-target-library-reorder.v1"
    )));
    game.validate_invariants()
        .expect("Dimir Machinations keeps the decision and event state machine valid");
}
