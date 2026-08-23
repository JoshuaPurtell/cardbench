//! Canonical-event state-machine contracts for the public Magic engine API.
//!
//! These assertions are deliberately about *transitions*, not just final game
//! state. A `CardBench` replay consumer only has the canonical event log, so
//! every automatic boundary that changes observable game state must be present
//! and chronologically usable.  The relevant public Magic rules are CR 106.4
//! (mana empties at step/phase boundaries), 111.6 (tokens cease to exist after
//! leaving the battlefield), 121.4/704.5b (empty-library loss), 703.1--703.4
//! (turn-based actions), 704.3 (state-based action timing), and 702.53a
//! (transmute's discard/search/reveal/shuffle sequence). This small RAV substrate
//! intentionally implements only the corresponding supported slices.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ContinuousChange, Duration, Effect, Game,
    GameEvent, Keyword, ManaCost, PlayerId, PolicyAction, Step, TokenSpec, Zone,
};

const BODY: &str = "EVENT-BODY";
const EFFECT_SOURCE: &str = "EVENT-EFFECT-SOURCE";
const TOKEN_SPELL: &str = "EVENT-TOKEN-SPELL";
const TRANSMUTER: &str = "EVENT-TRANSMUTER";
const MANA_VALUE_THREE: &str = "EVENT-MANA-VALUE-THREE";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn creature(id: &'static str, mana_cost: ManaCost, power: i16, toughness: i16) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: colors([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(power),
        toughness: Some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

fn instant(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["test-effect-source"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        creature(BODY, ManaCost::new(0), 2, 2),
        instant(EFFECT_SOURCE),
        CardDefinition {
            id: TOKEN_SPELL,
            name: TOKEN_SPELL,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["create-token"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec {
                    name: "Ephemeral 0/0",
                    is_legendary: false,
                    colors: colors([Color::Green]),
                    card_types: types([CardType::Creature]),
                    creature_subtypes: BTreeSet::new(),
                    keywords: vec![],
                    power: 0,
                    toughness: 0,
                },
                count: 1,
            }],
        },
        CardDefinition {
            id: TRANSMUTER,
            name: TRANSMUTER,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["transmute"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![],
        },
        creature(
            MANA_VALUE_THREE,
            ManaCost::with_colors(1, [Color::Green, Color::Green]),
            3,
            3,
        ),
    ]
}

fn game() -> Game {
    Game::new(definitions(), 2).expect("event-contract game initializes")
}

fn assert_invariants(game: &Game) {
    game.validate_invariants()
        .expect("every event-contract transition preserves engine invariants");
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> usize {
    events
        .iter()
        .position(predicate)
        .expect("canonical event required by the event-log contract")
}

fn advance_until(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..128 {
        if game.turn == turn && game.step == step {
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
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("take the ordinary draw");
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("the current priority holder may advance the trace");
        assert_invariants(game);
    }
    panic!("game did not reach the requested state-machine boundary");
}

/// Validates the semantic edge relation of every logged turn/step boundary.
///
/// The first observed marker may be an explicit `begin_game` marker, but every
/// following marker must be exactly the next `Step`, with a turn and active-seat
/// change only across Cleanup -> Untap.  This rejects the common replay bug of
/// emitting a step's automatic work under the preceding step marker.
fn assert_legal_step_trace(events: &[GameEvent], player_count: usize) {
    let steps: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::StepBegan {
                turn,
                active_player,
                step,
            } => Some((*turn, *active_player, *step)),
            _ => None,
        })
        .collect();
    assert!(steps.len() >= 2, "the trace must cross a step boundary");

    for pair in steps.windows(2) {
        let (turn, active_player, step) = pair[0];
        let (next_turn, next_active_player, next_step) = pair[1];
        let legal_empty_combat_skip =
            step == Step::DeclareAttackers && next_step == Step::EndOfCombat;
        assert!(
            legal_empty_combat_skip || next_step == step.next(),
            "only the fixed state-machine successor (or CR 508.8 empty-combat skip) may begin next"
        );
        if step == Step::Cleanup {
            assert_eq!(next_turn, turn + 1, "only cleanup starts a new turn");
            assert_eq!(
                next_active_player,
                PlayerId((active_player.0 + 1) % player_count),
                "only cleanup rotates the active seat"
            );
        } else {
            assert_eq!(next_turn, turn, "a non-cleanup edge cannot change turn");
            assert_eq!(
                next_active_player, active_player,
                "a non-cleanup edge cannot rotate the active seat"
            );
        }
    }
}

#[test]
fn step_boundary_precedes_automatic_work_and_the_trace_has_only_legal_edges() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = game();
    let body = game
        .put_on_battlefield(first, BODY)
        .expect("setup creature enters the battlefield");
    game.set_tapped_for_setup(body, true)
        .expect("setup creature can begin tapped");
    game.clear_event_log();

    game.begin_game().expect("prepared game begins");
    advance_until(&mut game, 2, Step::Upkeep);

    let events = &game.event_log;
    assert!(matches!(
        events.first(),
        Some(GameEvent::StepBegan {
            turn: 1,
            active_player,
            step: Step::Untap,
        }) if *active_player == first
    ));
    let untap_boundary = event_index(events, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                turn: 1,
                active_player,
                step: Step::Untap,
            } if *active_player == first
        )
    });
    let permanent_untapped = event_index(events, |event| {
        matches!(
            event,
            GameEvent::PermanentsUntapped { player, cards }
                if *player == first && cards == &vec![body]
        )
    });
    let upkeep_boundary = event_index(events, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                turn: 1,
                active_player,
                step: Step::Upkeep,
            } if *active_player == first
        )
    });
    assert!(untap_boundary < permanent_untapped);
    assert!(permanent_untapped < upkeep_boundary);
    assert_legal_step_trace(events, 2);
    assert_eq!(game.active_player, second);
    assert_invariants(&game);
}

#[test]
fn terminal_lifecycle_records_loss_then_one_final_game_end_without_later_actions() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = game();
    game.clear_event_log();

    game.draw_card(second, None)
        .expect("empty-library draw reaches the terminal transition");
    assert_eq!(
        game.event_log,
        vec![
            GameEvent::PlayerLost {
                player: second,
                reason: "attempted to draw from an empty library",
            },
            GameEvent::GameEnded {
                winner: Some(first),
            },
        ]
    );
    assert!(game.is_game_over());
    assert_eq!(game.winner(), Some(first));

    let before = game.event_log.clone();
    assert!(game.draw_card(first, None).is_err());
    assert_eq!(
        game.event_log, before,
        "a terminal game emits no new gameplay events"
    );
    assert_invariants(&game);
}

#[test]
fn policy_receipt_precedes_a_terminal_event_caused_by_its_submitted_draw() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = game();
    advance_until(&mut game, 2, Step::Upkeep);
    game.clear_event_log();

    game.submit_policy_move(second, "test.pass.v1", PolicyAction::PassPriority)
        .expect("upkeep holder may pass");
    game.submit_policy_move(first, "test.pass.v1", PolicyAction::PassPriority)
        .expect("second pass starts the draw replacement decision");
    let draw_decision = game
        .view_for_player(second)
        .expect("draw decision view")
        .draw_replacement_decision
        .expect("pending draw has a decision identity");
    game.submit_policy_move(
        second,
        "test.draw.v1",
        PolicyAction::Draw {
            decision: draw_decision,
            dredge: None,
        },
    )
    .expect("the ordinary empty-library draw ends the game");

    assert_eq!(
        game.event_log.last(),
        Some(&GameEvent::GameEnded {
            winner: Some(first),
        }),
        "terminal receipt must be the final event rather than preceding its accepted move"
    );
    assert!(matches!(
        game.event_log.get(game.event_log.len().saturating_sub(2)),
        Some(GameEvent::PolicyMoveSubmitted {
            player,
            kind: cardbench_magic_engine::PolicyMoveKind::Draw,
            ..
        }) if *player == second
    ));
    assert_invariants(&game);
}

#[test]
fn token_creation_sba_and_disappearance_are_complete_and_chronological() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = game();
    let spell = game
        .add_card(first, TOKEN_SPELL, Zone::Hand)
        .expect("token spell enters hand");

    game.cast_spell(
        first,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-cost token spell is cast");
    game.pass_priority(first)
        .expect("caster passes after casting");
    game.pass_priority(second)
        .expect("opponent passes and resolves");

    let events = &game.event_log;
    let (token_created, token) = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::TokenCreated { player, token } if *player == first => Some((index, *token)),
            _ => None,
        })
        .expect("the token's creation is logged");
    let spell_resolved = event_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    );
    let spell_to_graveyard = event_index(
        events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell),
    );
    let token_sba = event_index(events, |event| {
        matches!(
            event,
            GameEvent::StateBasedAction { card, reason }
                if *card == token && *reason == "creature has toughness zero or less"
        )
    });
    let token_ceased = event_index(
        events,
        |event| matches!(event, GameEvent::TokenCeasedToExist { token: ceased } if *ceased == token),
    );

    assert!(token_created < spell_resolved);
    assert!(spell_resolved < spell_to_graveyard);
    assert!(spell_to_graveyard < token_sba);
    assert!(token_sba < token_ceased);
    assert!(
        !events
            .iter()
            .any(|event| { matches!(event, GameEvent::CardMoved { card, .. } if *card == token) }),
        "the token lifecycle must not fabricate a normal card-zone move"
    );
    assert_eq!(game.zone_of(token), None);
    assert!(
        game.object(token).is_err(),
        "ceased token object is removed"
    );
    assert_invariants(&game);
}

#[test]
fn transmute_logs_the_required_reveal_and_shuffle_between_search_and_completion() {
    let first = PlayerId(0);
    let mut game = game();
    game.set_shuffle_seed(0x5EED)
        .expect("setup seed is accepted");
    let discarded = game
        .add_card(first, TRANSMUTER, Zone::Hand)
        .expect("transmute card enters hand");
    let found = game
        .add_card(first, MANA_VALUE_THREE, Zone::Library)
        .expect("matching mana-value card enters library");
    game.add_card(first, BODY, Zone::Library)
        .expect("a remaining library card makes the shuffle observable");
    game.grant_mana(first, Color::Blue, 3)
        .expect("setup mana pays transmute");
    game.clear_event_log();

    game.transmute(first, discarded, Some(found))
        .expect("legal transmute resolves");

    let events = &game.event_log;
    let discarded_to_graveyard = event_index(
        events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == discarded),
    );
    let found_to_hand = event_index(
        events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == found),
    );
    let reveal = event_index(events, |event| {
        matches!(
            event,
            GameEvent::CardRevealed {
                player,
                card,
                definition: MANA_VALUE_THREE,
            } if *player == first && *card == found
        )
    });
    let shuffle = event_index(events, |event| {
        matches!(
            event,
            GameEvent::LibraryShuffled { player, cards }
                if *player == first && *cards == 1
        )
    });
    let transmuted = event_index(events, |event| {
        matches!(
            event,
            GameEvent::Transmuted {
                player,
                discarded: logged_discarded,
                found: logged_found,
            } if *player == first && *logged_discarded == discarded && *logged_found == Some(found)
        )
    });
    assert!(discarded_to_graveyard < reveal);
    assert!(reveal < found_to_hand);
    assert!(found_to_hand < shuffle);
    assert!(shuffle < transmuted);
    assert_invariants(&game);
}

#[test]
fn transmute_requires_a_public_reveal_and_permits_no_result_search() {
    let first = PlayerId(0);
    let mut game = game();
    let discarded = game
        .add_card(first, TRANSMUTER, Zone::Hand)
        .expect("transmute card enters hand");
    let library_card = game
        .add_card(first, MANA_VALUE_THREE, Zone::Library)
        .expect("matching card may remain unfound in the library");
    game.grant_mana(first, Color::Blue, 3)
        .expect("setup mana pays transmute");
    game.clear_event_log();

    let no_result = game.transmute(first, discarded, None);

    assert!(
        no_result.is_ok(),
        "a hidden-zone quality search may legally find nothing: {no_result:?}"
    );

    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(library_card), Some(Zone::Library));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::CardRevealed { .. })),
        "no card can be revealed when the search finds nothing"
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::Transmuted {
                player,
                discarded: logged_discarded,
                found: None,
            } if *player == first && *logged_discarded == discarded
        )
    }));
    assert_invariants(&game);
}

#[test]
fn cleanup_logs_effect_expiry_after_its_boundary_and_clears_floating_mana() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = game();
    let source = game
        .add_card(first, EFFECT_SOURCE, Zone::Graveyard)
        .expect("instant source is available for the test effect");
    let target = game
        .put_on_battlefield(first, BODY)
        .expect("effect target begins on the battlefield");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ModifyPowerToughness {
            power: 1,
            toughness: 1,
        },
        Duration::EndOfTurn(game.turn),
    )
    .expect("current-turn effect is legal");
    assert_eq!(
        game.characteristics(target)
            .expect("target characteristics are visible")
            .power,
        Some(3)
    );
    game.clear_event_log();
    game.add_mana_from_action(first, Color::Red, 2)
        .expect("priority holder adds floating mana");
    advance_until(&mut game, 2, Step::Upkeep);

    let events = &game.event_log;
    let mana_added = event_index(events, |event| {
        matches!(
            event,
            GameEvent::ManaAdded {
                player,
                color: Color::Red,
                amount: 2,
            } if *player == first
        )
    });
    let beginning_of_combat = event_index(events, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                turn: 1,
                active_player,
                step: Step::BeginningOfCombat,
            } if *active_player == first
        )
    });
    let cleanup = event_index(events, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                turn: 1,
                active_player,
                step: Step::Cleanup,
            } if *active_player == first
        )
    });
    let expired = event_index(events, |event| {
        matches!(
            event,
            GameEvent::ContinuousEffectExpired {
                source: logged_source,
                target: logged_target,
                ..
            } if *logged_source == source && *logged_target == target
        )
    });
    let next_untap = event_index(events, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                turn: 2,
                active_player,
                step: Step::Untap,
            } if *active_player == second
        )
    });
    assert!(mana_added < beginning_of_combat);
    assert!(cleanup < expired);
    assert!(expired < next_untap);
    assert_legal_step_trace(events, 2);
    assert!(game.continuous_effects.is_empty());
    assert_eq!(
        game.characteristics(target)
            .expect("target characteristics are visible")
            .power,
        Some(2),
        "the expired end-of-turn effect no longer changes characteristics"
    );
    for player in &game.players {
        assert_eq!(
            player.mana_pool.total(),
            0,
            "mana cannot survive a step boundary"
        );
    }
    assert_invariants(&game);
}

#[test]
fn invariant_audit_rejects_an_externally_injected_terminal_event() {
    let mut game = game();
    game.event_log.push(GameEvent::GameEnded { winner: None });

    let error = game
        .validate_invariants()
        .expect_err("a continuing state cannot claim a terminal lifecycle event");
    assert!(
        error
            .to_string()
            .contains("canonical event log was mutated outside an engine transition")
    );
}
