//! Public transition-contract tests for the supported Magic turn state machine.
//!
//! These tests deliberately drive every transition through accepted public
//! actions, check the invariant boundary after each one, and inspect the
//! canonical event history. Opening-hand setup is intentionally external to
//! the turn machine; `begin_game` then starts player zero's first turn at
//! Untap and exposes priority only in Upkeep.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent,
    ManaCost, PlayerId, PolicyAction, RulesError, Step, Zone,
};

const PLAINS: &str = "STATE-PLAINS";
const ATTACKER: &str = "STATE-ATTACKER";
const BLOCKER: &str = "STATE-BLOCKER";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn card_types(card_types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    card_types.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: PLAINS,
            name: "State Plains",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: colors([Color::White]),
            card_types: card_types([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["basic-mana"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: ATTACKER,
            name: "State Attacker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: card_types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: BLOCKER,
            name: "State Blocker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: card_types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: Vec::<Effect>::new(),
        },
    ]
}

fn draw_deck() -> DeckList {
    DeckList {
        mainboard: vec![DeckEntry {
            card: PLAINS.to_owned(),
            // The tests cross two draw steps before turn three and retain a
            // margin so a library-loss rule cannot mask a transition failure.
            count: 8,
        }],
        sideboard: vec![],
    }
}

fn assert_invariants(game: &Game) {
    game.validate_invariants()
        .expect("each public transition must leave a valid game state");
}

fn pass_round(game: &mut Game) {
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
    for _ in 0..2 {
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("take the ordinary draw");
        }
        assert!(
            game.step.grants_priority(),
            "a public action cannot be requested in a no-priority step: {:?}",
            game.step
        );
        let player = game.priority;
        game.pass_priority(player)
            .expect("the current priority holder may pass");
        assert_invariants(game);
    }
}

fn advance_to(game: &mut Game, target_turn: u32, target_step: Step) {
    for _ in 0..64 {
        if game.turn == target_turn && game.step == target_step {
            return;
        }
        pass_round(game);
    }
    panic!(
        "turn state machine did not reach turn {target_turn} step {target_step:?}; reached turn {} step {:?}",
        game.turn, game.step
    );
}

fn declare_no_attackers(game: &mut Game) {
    assert_eq!(game.step, Step::DeclareAttackers);
    let active_player = game.active_player;
    game.submit_policy_move(
        active_player,
        "test.state-machine-empty-combat.v1",
        PolicyAction::DeclareAttackers { attackers: vec![] },
    )
    .expect("the active player may declare no attackers");
    assert_invariants(game);
}

fn step_events(game: &Game) -> Vec<(u32, PlayerId, Step)> {
    game.event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::StepBegan {
                turn,
                active_player,
                step,
            } => Some((*turn, *active_player, *step)),
            _ => None,
        })
        .collect()
}

#[test]
fn initial_slice_and_first_turn_transition_follow_the_declared_turn_machine() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    game.load_deck_into_library(first, &draw_deck())
        .expect("first deck loads");
    game.load_deck_into_library(second, &draw_deck())
        .expect("second deck loads");
    game.begin_game().expect("prepared game starts");

    assert_eq!(game.turn, 1);
    assert_eq!(game.active_player, first);
    assert_eq!(game.priority, first);
    assert_eq!(game.step, Step::Upkeep);
    assert_invariants(&game);

    // Four pass rounds reach declare attackers. The active player then makes
    // an explicit no-attacker declaration; after the next pass round, CR 508.8
    // skips declare blockers and combat damage. The final three rounds finish
    // the postcombat main phase, end step, and automatic cleanup/untap.
    for _ in 0..4 {
        pass_round(&mut game);
    }
    declare_no_attackers(&mut game);
    for _ in 0..4 {
        pass_round(&mut game);
    }
    assert_eq!(game.turn, 2);
    assert_eq!(game.active_player, second);
    assert_eq!(game.step, Step::Upkeep);
    assert_eq!(game.priority, second);
    assert_invariants(&game);

    assert_eq!(
        step_events(&game),
        vec![
            (1, first, Step::Untap),
            (1, first, Step::Upkeep),
            (1, first, Step::Draw),
            (1, first, Step::PrecombatMain),
            (1, first, Step::BeginningOfCombat),
            (1, first, Step::DeclareAttackers),
            (1, first, Step::EndOfCombat),
            (1, first, Step::PostcombatMain),
            (1, first, Step::End),
            (1, first, Step::Cleanup),
            (2, second, Step::Untap),
            (2, second, Step::Upkeep),
        ],
        "the first player has no turn-one draw and no turn phase is silently omitted"
    );

    pass_round(&mut game);
    assert_eq!(game.step, Step::Draw);
    assert!(
        game.view_for_player(second)
            .expect("draw decision view")
            .draw_replacement_pending,
        "a normal draw waits for its replacement-choice boundary"
    );
    assert_eq!(
        game.player(second)
            .expect("second player exists")
            .hand
            .len(),
        0,
        "the second player has not drawn before choosing a replacement"
    );
    game.resolve_pending_draw(second, None)
        .expect("the second player takes the ordinary draw");
    assert_eq!(
        game.player(second)
            .expect("second player exists")
            .hand
            .len(),
        1
    );
    assert_invariants(&game);
}

#[test]
fn begin_game_is_a_one_way_atomic_state_machine_transition() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    game.load_deck_into_library(first, &draw_deck())
        .expect("first deck loads");
    game.load_deck_into_library(second, &draw_deck())
        .expect("second deck loads");
    game.begin_game()
        .expect("prepared game starts exactly once");
    let before = (
        game.turn,
        game.step,
        game.active_player,
        game.priority,
        game.players.clone(),
        game.stack.clone(),
        game.event_log.clone(),
    );

    assert!(matches!(
        game.begin_game(),
        Err(RulesError::IllegalAction("the game has already begun"))
    ));
    assert_eq!(
        (
            game.turn,
            game.step,
            game.active_player,
            game.priority,
            game.players.clone(),
            game.stack.clone(),
            game.event_log.clone(),
        ),
        before,
        "re-entering the started state cannot duplicate turn-based actions or mutate game state"
    );
    assert_invariants(&game);
}

#[test]
fn untap_and_cleanup_are_never_exposed_as_priority_bearing_game_states() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    game.load_deck_into_library(first, &draw_deck())
        .expect("first deck loads");
    game.load_deck_into_library(second, &draw_deck())
        .expect("second deck loads");
    game.begin_game().expect("prepared game starts");

    assert!(!Step::Untap.grants_priority());
    assert!(!Step::Cleanup.grants_priority());
    assert!(Step::Upkeep.grants_priority());

    for _ in 0..4 {
        pass_round(&mut game);
    }
    declare_no_attackers(&mut game);
    for _ in 0..4 {
        pass_round(&mut game);
    }
    assert_eq!(game.step, Step::Upkeep);
    assert!(game.step.grants_priority());

    let cleanup = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::StepBegan {
                    turn: 1,
                    active_player,
                    step: Step::Cleanup,
                } if *active_player == first
            )
        })
        .expect("cleanup is recorded as an automatic step");
    let untap = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::StepBegan {
                    turn: 2,
                    active_player,
                    step: Step::Untap,
                } if *active_player == second
            )
        })
        .expect("untap is recorded as an automatic step");
    let upkeep = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::StepBegan {
                    turn: 2,
                    active_player,
                    step: Step::Upkeep,
                } if *active_player == second
            )
        })
        .expect("upkeep follows the automatic untap step");
    assert!(cleanup < untap && untap < upkeep);
    assert!(
        game.event_log[cleanup + 1..upkeep]
            .iter()
            .all(|event| !matches!(event, GameEvent::PriorityPassed { .. })),
        "no player may pass priority between automatic cleanup/untap events and the next upkeep"
    );
    assert_invariants(&game);
}

#[test]
fn priority_handoff_requires_the_holder_and_two_passes_advance_an_empty_stack() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    game.load_deck_into_library(first, &draw_deck())
        .expect("first deck loads");
    game.load_deck_into_library(second, &draw_deck())
        .expect("second deck loads");
    game.begin_game().expect("prepared game starts");
    advance_to(&mut game, 1, Step::PrecombatMain);
    let priority_passes_before = game
        .event_log
        .iter()
        .filter(|event| matches!(event, GameEvent::PriorityPassed { .. }))
        .count();
    let before = (
        game.turn,
        game.step,
        game.active_player,
        game.priority,
        game.players.clone(),
        game.stack.clone(),
        game.event_log.clone(),
    );

    assert!(matches!(
        game.pass_priority(second),
        Err(RulesError::Priority {
            expected,
            actual,
        }) if expected == first && actual == second
    ));
    assert_eq!(
        (
            game.turn,
            game.step,
            game.active_player,
            game.priority,
            game.players.clone(),
            game.stack.clone(),
            game.event_log.clone(),
        ),
        before,
        "a rejected wrong-priority pass is atomic"
    );
    assert_invariants(&game);

    game.pass_priority(first)
        .expect("the active priority holder passes to the opponent");
    assert_eq!(game.step, Step::PrecombatMain);
    assert_eq!(game.priority, second);
    assert_invariants(&game);

    game.pass_priority(second)
        .expect("the consecutive second pass advances the empty-stack step");
    assert_eq!(game.step, Step::BeginningOfCombat);
    assert_eq!(game.priority, first);
    assert_eq!(game.active_player, first);
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::PriorityPassed { .. }))
            .count(),
        priority_passes_before + 2
    );
    assert_invariants(&game);
}

#[test]
fn legal_policy_combat_declarations_follow_the_supported_turn_based_flow() {
    let attacker_controller = PlayerId(0);
    let defending_player = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    game.load_deck_into_library(attacker_controller, &draw_deck())
        .expect("attacker deck loads");
    game.load_deck_into_library(defending_player, &draw_deck())
        .expect("defender deck loads");
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker enters on turn one");
    let blocker = game
        .put_on_battlefield(defending_player, BLOCKER)
        .expect("blocker enters on turn one");
    game.begin_game().expect("prepared game starts");
    assert_invariants(&game);

    advance_to(&mut game, 3, Step::DeclareAttackers);
    let attacker_view = game
        .view_for_player(attacker_controller)
        .expect("attacker view is available");
    assert_eq!(attacker_view.decision_player, attacker_controller);
    assert_eq!(attacker_view.priority, attacker_controller);
    assert!(
        attacker_view
            .own_battlefield
            .iter()
            .any(|card| card.id == attacker && card.can_attack),
        "the turn-one attacker is no longer summoning sick on turn three"
    );

    game.submit_policy_move(
        attacker_controller,
        "test.state-machine-combat.v1",
        PolicyAction::DeclareAttackers {
            attackers: vec![attacker],
        },
    )
    .expect("active player makes the legal turn-based attacker declaration");
    assert!(game.object(attacker).expect("attacker exists").tapped);
    assert_invariants(&game);

    pass_round(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    let blocker_view = game
        .view_for_player(defending_player)
        .expect("defender view is available");
    assert_eq!(blocker_view.decision_player, defending_player);
    assert!(!blocker_view.blockers_declared);

    game.submit_policy_move(
        defending_player,
        "test.state-machine-combat.v1",
        PolicyAction::DeclareBlockers {
            assignments: vec![CombatBlock { attacker, blocker }],
        },
    )
    .expect("defending player makes the legal turn-based blocker declaration");
    assert_eq!(game.priority, attacker_controller);
    assert_invariants(&game);

    pass_round(&mut game);
    assert_eq!(game.step, Step::CombatDamage);
    assert_eq!(
        game.player(defending_player).expect("defender exists").life,
        20,
        "a blocked non-trample attacker does not damage the defending player"
    );
    assert_eq!(game.zone_of(blocker), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(attacker), Some(Zone::Battlefield));
    assert_invariants(&game);
}
