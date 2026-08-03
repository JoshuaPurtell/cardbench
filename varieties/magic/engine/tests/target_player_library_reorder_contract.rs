//! Core contract for controller-private reordering of a target player's library.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection,
    DecisionVisibility, Effect, Game, GameEvent, ManaCost, PlayerId, PolicyAction, Target, Zone,
};

const SPELL: &str = "TST-TARGET-LIBRARY-REORDER";
const BOTTOM: &str = "TST-BOTTOM";
const FIRST: &str = "TST-FIRST";
const SECOND: &str = "TST-SECOND";
const THIRD: &str = "TST-THIRD";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["target-player-private-library-reorder"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new(
        [
            definition(
                SPELL,
                vec![Effect::LookAtTopCardsOfTargetPlayerAndReorder { count: 3 }],
            ),
            definition(BOTTOM, vec![]),
            definition(FIRST, vec![]),
            definition(SECOND, vec![]),
            definition(THIRD, vec![]),
        ],
        2,
    )
    .expect("synthetic catalog builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
#[allow(clippy::too_many_lines)] // One transcript proves privacy, exhaustive choice, and atomic stale protection.
fn target_player_library_reorder_is_private_exhaustive_and_stack_bound() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), SPELL, Zone::Hand)
        .expect("spell enters hand");
    let bottom = game
        .add_card(PlayerId(1), BOTTOM, Zone::Library)
        .expect("bottom enters library");
    let first = game
        .add_card(PlayerId(1), FIRST, Zone::Library)
        .expect("first enters library");
    let second = game
        .add_card(PlayerId(1), SECOND, Zone::Library)
        .expect("second enters library");
    let third = game
        .add_card(PlayerId(1), THIRD, Zone::Library)
        .expect("third enters library");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("targeted reorder spell casts");
    pass_pair(&mut game);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private reorder decision opens");
    assert_eq!(decision.kind, DecisionKind::TargetPlayerLibraryTopReorder);
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!(decision.min_selections, 3);
    assert_eq!(decision.max_selections, 3);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![third, second, first],
        "the spell controller alone receives the target library's top-first snapshot"
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("target view")
            .pending_decision
            .is_none(),
        "the library owner cannot inspect the controller-private candidate projection"
    );
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardRevealed { card, .. }
            if [first, second, third].contains(card)
    )));

    let before = (
        game.players.clone(),
        game.stack.clone(),
        game.canonical_event_log(),
    );
    assert!(
        game.submit_decision(
            PlayerId(1),
            decision.id,
            DecisionSelection::TargetPlayerLibraryTopReorder {
                top: vec![second],
                bottom: vec![first, third],
            },
        )
        .is_err(),
        "only the resolving controller can submit the hidden choice"
    );
    assert_eq!(
        (
            game.players.clone(),
            game.stack.clone(),
            game.canonical_event_log()
        ),
        before,
        "a foreign choice cannot mutate zones, stack, or public history"
    );
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::TargetPlayerLibraryTopReorder {
                top: vec![second, second],
                bottom: vec![first],
            },
        )
        .is_err(),
        "the private partition must contain each inspected object exactly once"
    );
    assert_eq!(
        (
            game.players.clone(),
            game.stack.clone(),
            game.canonical_event_log()
        ),
        before,
        "a malformed partition rolls back atomically"
    );

    game.submit_policy_move(
        PlayerId(0),
        "target-library-reorder-contract.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::TargetPlayerLibraryTopReorder {
                top: vec![second],
                bottom: vec![first, third],
            },
        },
    )
    .expect("policy-submitted exhaustive target-library partition resolves");

    println!(
        "target library reorder trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.players[PlayerId(1).0].library,
        vec![first, third, bottom, second],
        "bottom is submitted bottom-to-top while top is submitted top-to-bottom"
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
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
            && *source == spell
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
        } if *controller == PlayerId(0) && *source == spell && *target == PlayerId(1)
    )));
    game.validate_invariants()
        .expect("private target-library reorder preserves state-machine invariants");
}
