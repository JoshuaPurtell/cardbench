//! Adversarial contracts for draw replacements and multiplayer combat/loss.
//!
//! These probes make policies submit the same draw decisions that a full-game
//! runner uses.  They specifically guard against treating a pending draw
//! replacement as an ordinary priority window, which would allow a spell to
//! interleave with an unresolved replacement choice and corrupt the turn
//! state machine.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent,
    Keyword, ManaCost, PlayerId, PolicyAction, PolicyMoveKind, RulesError, Step, Zone,
};

const FILLER: &str = "REPLACEMENT-FILLER";
const DREDGE_TWO: &str = "REPLACEMENT-DREDGE-TWO";
const DREDGE_THREE: &str = "REPLACEMENT-DREDGE-THREE";
const FREE_INSTANT: &str = "REPLACEMENT-FREE-INSTANT";
const FINISHER: &str = "REPLACEMENT-FINISHER";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(card_types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    card_types.into_iter().collect()
}

fn creature(
    id: &'static str,
    power: i16,
    toughness: i16,
    keywords: Vec<Keyword>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: colors([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[("base-characteristics")],
        power: Some(power),
        toughness: Some(toughness),
        keywords,
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: FILLER,
            name: "Replacement Filler",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["fixture"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        creature(DREDGE_TWO, 1, 1, vec![Keyword::Dredge(2)]),
        creature(DREDGE_THREE, 1, 1, vec![Keyword::Dredge(3)]),
        CardDefinition {
            id: FREE_INSTANT,
            name: "Replacement Free Instant",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["gain-life"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::GainLifeController { amount: 1 }],
        },
        creature(FINISHER, 20, 20, vec![]),
    ]
}

fn game(player_count: usize) -> Game {
    Game::new(definitions(), player_count).expect("contract game initializes")
}

fn assert_invariants(game: &Game) {
    game.validate_invariants()
        .expect("every accepted transition must preserve the state machine");
}

fn living_players(game: &Game) -> usize {
    game.players.iter().filter(|player| !player.lost).count()
}

fn submit_empty_declaration_if_needed(game: &mut Game) {
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .attackers_declared
    {
        let player = game.next_policy_player();
        game.submit_policy_move(
            player,
            "test.replacement-multiplayer.v1",
            PolicyAction::DeclareAttackers { attackers: vec![] },
        )
        .expect("empty attackers are an explicit turn action");
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .blockers_declared
    {
        let player = game.next_policy_player();
        game.submit_policy_move(
            player,
            "test.replacement-multiplayer.v1",
            PolicyAction::DeclareBlockers {
                assignments: vec![],
            },
        )
        .expect("empty blockers are an explicit turn action");
    }
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..512 {
        if game.turn == turn && game.step == step {
            return;
        }
        submit_empty_declaration_if_needed(game);
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.submit_policy_move(
                player,
                "test.replacement-multiplayer.v1",
                PolicyAction::Draw { dredge: None },
            )
            .expect("ordinary draw advances the fixture");
            assert_invariants(game);
            continue;
        }
        let player = game.priority;
        game.submit_policy_move(
            player,
            "test.replacement-multiplayer.v1",
            PolicyAction::PassPriority,
        )
        .expect("priority holder advances the fixture");
        assert_invariants(game);
    }
    panic!(
        "fixture did not reach turn {turn}, step {step:?}; reached turn {}, step {:?}",
        game.turn, game.step
    );
}

fn pass_all_current_players(game: &mut Game) {
    let count = living_players(game);
    for _ in 0..count {
        let player = game.priority;
        game.submit_policy_move(
            player,
            "test.replacement-multiplayer.v1",
            PolicyAction::PassPriority,
        )
        .expect("every surviving player may pass once");
        assert_invariants(game);
    }
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> usize {
    events
        .iter()
        .position(predicate)
        .expect("required event is present in the canonical log")
}

#[test]
#[allow(clippy::too_many_lines)] // The full rejected-and-accepted draw transcript is the contract.
fn policy_draw_replacement_rejects_interleaved_priority_and_invalid_choices_atomically() {
    let first = PlayerId(0);
    let deciding_player = PlayerId(1);
    let mut game = game(2);
    let instant = game
        .add_card(deciding_player, FREE_INSTANT, Zone::Hand)
        .expect("instant is available for the illegal interleaving probe");
    let dredge_two = game
        .add_card(deciding_player, DREDGE_TWO, Zone::Graveyard)
        .expect("legal dredger is in the graveyard");
    let dredge_three = game
        .add_card(deciding_player, DREDGE_THREE, Zone::Graveyard)
        .expect("unpayable dredger is in the graveyard");
    let milled_first = game
        .add_card(deciding_player, FILLER, Zone::Library)
        .expect("first mill card enters library");
    let milled_second = game
        .add_card(deciding_player, FILLER, Zone::Library)
        .expect("second mill card enters library");

    game.begin_game().expect("game starts at first upkeep");
    advance_to(&mut game, 2, Step::Draw);
    let deciding_view = game
        .view_for_player(deciding_player)
        .expect("deciding player has a view");
    assert!(deciding_view.draw_replacement_pending);
    assert_eq!(deciding_view.decision_player, deciding_player);
    assert_eq!(
        deciding_view
            .dredge_candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![dredge_two],
        "only a payable owned dredge card is exposed to the policy"
    );
    let opponent_view = game
        .view_for_player(first)
        .expect("opponent has a visibility-limited view");
    assert!(!opponent_view.draw_replacement_pending);
    assert!(opponent_view.dredge_candidates.is_empty());

    game.clear_event_log();
    let before_players = game.players.clone();
    let before_stack = game.stack.clone();
    let before_events = game.event_log.clone();
    assert!(matches!(
        game.submit_policy_move(
            deciding_player,
            "test.replacement-multiplayer.v1",
            PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                card: instant,
                targets: vec![],
                convoke: vec![],
            }),
        ),
        Err(RulesError::IllegalAction(
            "the draw replacement decision must resolve before priority actions"
        ))
    ));
    assert_eq!(game.players, before_players);
    assert_eq!(game.stack, before_stack);
    assert_eq!(game.event_log, before_events);
    assert_eq!(game.zone_of(instant), Some(Zone::Hand));
    assert!(
        game.view_for_player(deciding_player)
            .expect("replacement remains visible after rejection")
            .draw_replacement_pending
    );
    assert_invariants(&game);

    let before_players = game.players.clone();
    let before_stack = game.stack.clone();
    let before_events = game.event_log.clone();
    assert!(matches!(
        game.submit_policy_move(
            deciding_player,
            "test.replacement-multiplayer.v1",
            PolicyAction::Draw {
                dredge: Some(dredge_three),
            },
        ),
        Err(RulesError::IllegalAction(
            "dredge replacement requires at least that many cards in library"
        ))
    ));
    assert_eq!(game.players, before_players);
    assert_eq!(game.stack, before_stack);
    assert_eq!(game.event_log, before_events);
    assert_eq!(game.zone_of(dredge_three), Some(Zone::Graveyard));
    assert!(
        game.view_for_player(deciding_player)
            .expect("replacement remains visible after rejected dredge")
            .draw_replacement_pending
    );
    assert_invariants(&game);

    game.submit_policy_move(
        deciding_player,
        "test.replacement-multiplayer.v1",
        PolicyAction::Draw {
            dredge: Some(dredge_two),
        },
    )
    .expect("policy submits its visible payable dredge choice");
    assert_eq!(game.zone_of(dredge_two), Some(Zone::Hand));
    assert_eq!(game.zone_of(milled_first), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(milled_second), Some(Zone::Graveyard));
    assert!(
        !game
            .view_for_player(deciding_player)
            .expect("replacement decision completed")
            .draw_replacement_pending
    );

    let events = &game.event_log;
    let first_mill = event_index(
        events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == milled_second),
    );
    let second_mill = event_index(
        events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == milled_first),
    );
    let dredger_to_hand = event_index(
        events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == dredge_two),
    );
    let dredged = event_index(
        events,
        |event| matches!(event, GameEvent::Dredged { player, card, count: 2 } if *player == deciding_player && *card == dredge_two),
    );
    let receipt = event_index(
        events,
        |event| matches!(event, GameEvent::PolicyMoveSubmitted { player, kind: PolicyMoveKind::Draw, .. } if *player == deciding_player),
    );
    assert!(first_mill < second_mill);
    assert!(second_mill < dredger_to_hand);
    assert!(dredger_to_hand < dredged);
    assert!(dredged < receipt);
    assert_eq!(
        events.len(),
        5,
        "rejected proposals never receive a receipt"
    );
    assert_invariants(&game);
}

#[test]
fn policy_may_take_the_normal_draw_even_when_dredge_is_available() {
    let deciding_player = PlayerId(1);
    let mut game = game(2);
    let dredger = game
        .add_card(deciding_player, DREDGE_TWO, Zone::Graveyard)
        .expect("dredger is in the graveyard");
    game.add_card(deciding_player, FILLER, Zone::Library)
        .expect("library makes dredge visible too");
    let normal_draw = game
        .add_card(deciding_player, FILLER, Zone::Library)
        .expect("normal draw card is on top of the fixture library");

    game.begin_game().expect("game starts");
    advance_to(&mut game, 2, Step::Draw);
    assert!(
        game.view_for_player(deciding_player)
            .expect("draw view")
            .dredge_candidates
            .iter()
            .any(|card| card.id == dredger)
    );
    game.clear_event_log();

    game.submit_policy_move(
        deciding_player,
        "test.replacement-multiplayer.v1",
        PolicyAction::Draw { dredge: None },
    )
    .expect("a policy can decline its available dredge replacement");
    assert_eq!(game.zone_of(normal_draw), Some(Zone::Hand));
    assert_eq!(game.zone_of(dredger), Some(Zone::Graveyard));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::Dredged { .. })),
        "ordinary draw must not fabricate a dredge event"
    );
    assert!(matches!(
        game.event_log.as_slice(),
        [
            GameEvent::CardMoved { card, to: Zone::Hand },
            GameEvent::PolicyMoveSubmitted { player, kind: PolicyMoveKind::Draw, .. },
        ] if *card == normal_draw && *player == deciding_player
    ));
    assert_invariants(&game);
}

#[test]
fn setup_only_deck_and_opening_hand_transitions_reject_runtime_injection_atomically() {
    let player = PlayerId(0);
    let mut game = game(2);
    let deck = DeckList {
        mainboard: vec![DeckEntry {
            card: FILLER.to_owned(),
            count: 2,
        }],
        sideboard: vec![],
    };
    game.load_deck_into_library(player, &deck)
        .expect("deck setup succeeds before the turn machine starts");
    game.draw_opening_hand(player, 1)
        .expect("opening-hand setup succeeds before the turn machine starts");
    game.begin_game().expect("game starts after setup");
    game.clear_event_log();

    let before_players = game.players.clone();
    assert!(matches!(
        game.draw_opening_hand(player, 1),
        Err(RulesError::IllegalAction(
            "an opening hand may be drawn only before the game begins"
        ))
    ));
    assert_eq!(
        game.players, before_players,
        "a rejected runtime opening-hand proposal cannot move a hidden card"
    );
    assert!(
        game.event_log.is_empty(),
        "a rejected runtime setup proposal has no OpeningHandDrawn receipt"
    );

    let empty_runtime_player = PlayerId(1);
    assert!(matches!(
        game.load_deck_into_library(empty_runtime_player, &deck),
        Err(RulesError::IllegalAction(
            "a deck may be loaded only before the game begins"
        ))
    ));
    assert!(game.players[empty_runtime_player.0].library.is_empty());
    assert!(
        game.event_log.is_empty(),
        "a rejected runtime deck load has no DeckLoaded or LibraryShuffled receipt"
    );
    assert_invariants(&game);
}

#[test]
fn a_second_opening_hand_cannot_be_appended_during_setup() {
    let player = PlayerId(0);
    let mut game = game(2);
    let deck = DeckList {
        mainboard: vec![DeckEntry {
            card: FILLER.to_owned(),
            count: 2,
        }],
        sideboard: vec![],
    };
    game.load_deck_into_library(player, &deck)
        .expect("deck setup succeeds");
    game.draw_opening_hand(player, 1)
        .expect("the first opening hand succeeds");
    let before_players = game.players.clone();
    let before_events = game.event_log.clone();

    assert!(matches!(
        game.draw_opening_hand(player, 1),
        Err(RulesError::IllegalAction(
            "an opening hand requires an empty hand"
        ))
    ));
    assert_eq!(game.players, before_players);
    assert_eq!(game.event_log, before_events);
    assert_invariants(&game);
}

#[test]
fn active_player_lost_while_taking_a_pending_draw_starts_the_next_survivors_turn() {
    let eliminated = PlayerId(0);
    let next_survivor = PlayerId(1);
    let final_survivor = PlayerId(2);
    let mut game = game(3);
    let departing_permanent = game
        .put_on_battlefield(eliminated, FINISHER)
        .expect("departing player's object gives the cleanup transition work");
    game.add_card(next_survivor, FILLER, Zone::Library)
        .expect("next survivor can later draw");
    game.add_card(final_survivor, FILLER, Zone::Library)
        .expect("last survivor can later draw");

    game.begin_game().expect("multiplayer game starts");
    advance_to(&mut game, 1, Step::Draw);
    assert!(
        game.view_for_player(eliminated)
            .expect("active draw view")
            .draw_replacement_pending
    );
    game.clear_event_log();

    game.submit_policy_move(
        eliminated,
        "test.replacement-multiplayer.v1",
        PolicyAction::Draw { dredge: None },
    )
    .expect("empty-library draw accepts the decision before applying the loss rule");
    assert!(game.player(eliminated).expect("seat is retained").lost);
    assert_eq!(game.active_player, next_survivor);
    assert_eq!(game.priority, next_survivor);
    assert_eq!(game.next_policy_player(), next_survivor);
    assert_eq!(game.turn, 2);
    assert_eq!(game.step, Step::Upkeep);
    assert_eq!(game.zone_of(departing_permanent), None);
    assert!(game.object(departing_permanent).is_err());
    assert!(!game.is_game_over(), "two survivors continue the game");

    let events = &game.event_log;
    let loss = event_index(
        events,
        |event| matches!(event, GameEvent::PlayerLost { player, reason } if *player == eliminated && *reason == "attempted to draw from an empty library"),
    );
    let object_left = event_index(
        events,
        |event| matches!(event, GameEvent::ObjectLeftGame { object, owner } if *object == departing_permanent && *owner == eliminated),
    );
    let untap = event_index(
        events,
        |event| matches!(event, GameEvent::StepBegan { turn: 2, active_player, step: Step::Untap } if *active_player == next_survivor),
    );
    let upkeep = event_index(
        events,
        |event| matches!(event, GameEvent::StepBegan { turn: 2, active_player, step: Step::Upkeep } if *active_player == next_survivor),
    );
    let receipt = event_index(
        events,
        |event| matches!(event, GameEvent::PolicyMoveSubmitted { player, kind: PolicyMoveKind::Draw, .. } if *player == eliminated),
    );
    assert!(loss < object_left);
    assert!(object_left < untap);
    assert!(untap < upkeep);
    assert!(upkeep < receipt);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::GameEnded { .. })),
        "continuing multiplayer loss does not emit a terminal event"
    );
    assert_invariants(&game);
}

#[test]
fn defender_lost_to_combat_damage_is_skipped_by_later_priority_and_turn_transitions() {
    let attacker_controller = PlayerId(0);
    let defeated_defender = PlayerId(1);
    let remaining_defender = PlayerId(2);
    let mut game = game(3);
    let attacker = game
        .put_on_battlefield(attacker_controller, FINISHER)
        .expect("attacker enters before the game begins");
    for player in [attacker_controller, defeated_defender, remaining_defender] {
        for _ in 0..8 {
            game.add_card(player, FILLER, Zone::Library)
                .expect("each player has enough cards for the setup turns");
        }
    }
    // This is fixture setup; combat itself is driven solely through submitted
    // actions below. A 20-power attacker turns the actual damage transition
    // into an elimination transition without adding an unsupported spell.
    game.players[defeated_defender.0].life = 1;

    game.begin_game().expect("multiplayer game starts");
    advance_to(&mut game, 4, Step::DeclareAttackers);
    assert_eq!(game.active_player, attacker_controller);
    game.clear_event_log();

    game.submit_policy_move(
        attacker_controller,
        "test.replacement-multiplayer.v1",
        PolicyAction::DeclareAttackers {
            attackers: vec![attacker],
        },
    )
    .expect("active player declares the legal attacker");
    pass_all_current_players(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    assert_eq!(game.next_policy_player(), defeated_defender);
    game.submit_policy_move(
        defeated_defender,
        "test.replacement-multiplayer.v1",
        PolicyAction::DeclareBlockers {
            assignments: Vec::<CombatBlock>::new(),
        },
    )
    .expect("defender explicitly declines blockers");
    pass_all_current_players(&mut game);

    assert_eq!(game.step, Step::CombatDamage);
    assert!(game.player(defeated_defender).expect("seat remains").lost);
    assert_eq!(game.priority, attacker_controller);
    assert_ne!(game.next_policy_player(), defeated_defender);
    assert_eq!(
        game.player(defeated_defender).expect("seat remains").life,
        -19
    );
    let before_events = game.event_log.clone();
    assert!(matches!(
        game.submit_policy_move(
            defeated_defender,
            "test.replacement-multiplayer.v1",
            PolicyAction::PassPriority,
        ),
        Err(RulesError::IllegalAction("an eliminated player cannot act"))
    ));
    assert_eq!(game.event_log, before_events);

    let damage = event_index(
        &game.event_log,
        |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player, amount: 20 } if *source == attacker && *player == defeated_defender),
    );
    let loss = event_index(
        &game.event_log,
        |event| matches!(event, GameEvent::PlayerLost { player, reason } if *player == defeated_defender && *reason == "life total is zero or less"),
    );
    assert!(damage < loss, "damage is logged before its SBA loss result");
    assert_invariants(&game);

    pass_all_current_players(&mut game);
    assert_eq!(game.step, Step::EndOfCombat);
    assert_eq!(game.priority, attacker_controller);
    assert_eq!(living_players(&game), 2);
    assert_invariants(&game);
}
