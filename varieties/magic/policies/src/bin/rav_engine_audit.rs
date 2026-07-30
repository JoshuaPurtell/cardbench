//! Adversarial public-API probes for engine bugs.
//!
//! This is intentionally not a policy tournament. Each probe asks whether a
//! core rule boundary fails loudly and leaves a valid state behind.

use std::process::ExitCode;

use cardbench_magic_engine::{
    CastRequest, Color, CombatBlock, DeckEntry, DeckList, DeckRules, Game, PlayerId, PolicyAction,
    Step, Target, Zone,
};
use cardbench_magic_policies::{DeckMatchConfig, DeckMatchTermination, run_rav_deck_matchup};
use cardbench_magic_rav::card_definitions;

#[derive(Debug)]
struct Finding {
    code: &'static str,
    detail: String,
}

const POLICY_MATRIX_SEED_COUNT: u64 = 16;
const POLICY_MATRIX_MATCH_COUNT: u64 = 4 * POLICY_MATRIX_SEED_COUNT;

fn main() -> ExitCode {
    let mut findings = [
        probe_terminal_game_actions(),
        probe_terminal_game_draw(),
        probe_rejected_policy_atomicity(),
        probe_opening_hand_event_integrity(),
        probe_sideboard_copy_limit(),
        probe_multiplayer_elimination(),
        probe_eliminated_player_priority(),
        probe_active_player_elimination_continues_multiplayer(),
        probe_transmute_sorcery_timing(),
        probe_out_of_window_dredge(),
        probe_unsupported_spell_front_face(),
        probe_combat_with_dead_token(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    findings.extend(probe_policy_matchup_matrix());
    println!("schema_version=cardbench.magic.engine-audit.v1");
    println!("policy_matrix_pairing_count=4");
    println!("policy_matrix_seed_count={POLICY_MATRIX_SEED_COUNT}");
    println!("policy_matrix_match_count={POLICY_MATRIX_MATCH_COUNT}");
    println!("finding_count={}", findings.len());
    for finding in &findings {
        println!("finding=code:{} detail:{}", finding.code, finding.detail);
    }
    if findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Runs every public Boros/Selesnya policy pairing across deterministic shuffled
/// decks. It exercises actual policy submissions, while preserving the distinction
/// between an engine finding and a policy/harness failure in the finding code.
fn probe_policy_matchup_matrix() -> Vec<Finding> {
    const PAIRINGS: [(&str, &str); 4] = [
        ("rav_boros_helix", "rav_selesnya_convoke"),
        ("rav_boros_helix", "rav_selesnya_siege"),
        ("rav_boros_char_control", "rav_selesnya_convoke"),
        ("rav_boros_char_control", "rav_selesnya_siege"),
    ];
    let mut findings = Vec::new();
    for (deck_p0, deck_p1) in PAIRINGS {
        for shuffle_seed in 0..POLICY_MATRIX_SEED_COUNT {
            let label = format!("{deck_p0}-vs-{deck_p1}-seed-{shuffle_seed}");
            match run_rav_deck_matchup(
                DeckMatchConfig {
                    shuffle_seed,
                    ..DeckMatchConfig::default()
                },
                deck_p0,
                deck_p1,
            ) {
                Ok(result) if !result.engine_findings.is_empty() => findings.push(Finding {
                    code: "policy-matrix-engine-finding",
                    detail: format!("{label}: {:?}", result.engine_findings),
                }),
                Ok(result) if !matches!(result.termination, DeckMatchTermination::Winner(_)) => {
                    findings.push(Finding {
                        code: "policy-matrix-nonwinning-run",
                        detail: format!("{label}: {:?}", result.termination),
                    });
                }
                Ok(_) => {}
                Err(error) => findings.push(Finding {
                    code: "policy-matrix-setup-failed",
                    detail: format!("{label}: {error}"),
                }),
            }
        }
    }
    findings
}

/// A finished game may not accept a new gameplay action. This probe uses Char
/// because the executable slice gives it deterministic player damage.
fn probe_terminal_game_actions() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    game.players[1].life = 4;
    let char = game.add_card(PlayerId(0), "RAV-CHAR", Zone::Hand).ok()?;
    game.grant_mana(PlayerId(0), Color::Red, 3).ok()?;
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
        },
    )
    .ok()?;
    game.pass_priority(PlayerId(1)).ok()?;
    game.pass_priority(PlayerId(0)).ok()?;
    if !game.is_game_over() || !game.players[1].lost {
        return Some(Finding {
            code: "audit.setup-terminal-game-failed",
            detail: "Char resolution did not end the prepared game".to_owned(),
        });
    }
    let land = game.add_card(PlayerId(0), "RAV-PLAINS", Zone::Hand).ok()?;
    match game.submit_policy_move(
        PlayerId(0),
        "audit.terminal-game.v1",
        PolicyAction::PlayLand { card: land },
    ) {
        Ok(()) => Some(Finding {
            code: "game-over-accepts-gameplay-action",
            detail: "engine accepted PlayLand after PlayerLost".to_owned(),
        }),
        Err(_) => None,
    }
}

/// Priority-bearing actions and direct public draw APIs need separate terminal
/// guards: only the former happens to flow through `require_priority` today.
fn probe_terminal_game_draw() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    game.players[1].life = 4;
    let char = game.add_card(PlayerId(0), "RAV-CHAR", Zone::Hand).ok()?;
    let deferred_draw = game
        .add_card(PlayerId(0), "RAV-MOUNTAIN", Zone::Library)
        .ok()?;
    game.grant_mana(PlayerId(0), Color::Red, 3).ok()?;
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
        },
    )
    .ok()?;
    game.pass_priority(PlayerId(1)).ok()?;
    game.pass_priority(PlayerId(0)).ok()?;
    if !game.is_game_over() {
        return Some(Finding {
            code: "audit.setup-terminal-game-failed",
            detail: "Char resolution did not end the prepared game".to_owned(),
        });
    }
    let draw = game.draw_card(PlayerId(0), None);
    if draw.is_ok() && game.zone_of(deferred_draw) == Some(Zone::Hand) {
        return Some(Finding {
            code: "game-over-allows-direct-draw-mutation",
            detail: "draw_card moved a library card after PlayerLost".to_owned(),
        });
    }
    None
}

/// A rejected policy proposal must neither write an accepted-action event nor
/// move the proposed card. This guards the policy-to-engine transaction boundary.
fn probe_rejected_policy_atomicity() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    let land = game.add_card(PlayerId(1), "RAV-PLAINS", Zone::Hand).ok()?;
    game.clear_event_log();
    let rejected = game.submit_policy_move(
        PlayerId(1),
        "audit.atomicity.v1",
        PolicyAction::PlayLand { card: land },
    );
    if rejected.is_ok()
        || game.zone_of(land) != Some(Zone::Hand)
        || !game.event_log.is_empty()
        || game.validate_invariants().is_err()
    {
        return Some(Finding {
            code: "rejected-policy-action-is-not-atomic",
            detail: format!(
                "result={rejected:?} zone={:?} events={:?}",
                game.zone_of(land),
                game.event_log
            ),
        });
    }
    None
}

/// The opening-hand event is an observable claim. A short library must not
/// report that all requested cards were drawn after deck-out stopped the draw.
fn probe_opening_hand_event_integrity() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    let short_deck = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 2,
        }],
        sideboard: Vec::new(),
    };
    game.load_deck_into_library(PlayerId(0), &short_deck).ok()?;
    let result = game.draw_opening_hand(PlayerId(0), 7);
    let opening_event_claims_seven = game
        .canonical_event_log()
        .iter()
        .any(|event| event.contains("OpeningHandDrawn") && event.contains("cards: 7"));
    if result.is_ok()
        && game.players[0].lost
        && game.players[0].hand.len() == 2
        && opening_event_claims_seven
    {
        return Some(Finding {
            code: "opening-hand-event-overstates-cards-drawn",
            detail: "a two-card library returned Ok and logged OpeningHandDrawn { cards: 7 }"
                .to_owned(),
        });
    }
    None
}

/// The four-copy limit applies to the combined deck and sideboard. Keeping
/// separate counters lets a reference fixture create an illegal five-Char list.
fn probe_sideboard_copy_limit() -> Option<Finding> {
    let catalog = card_definitions()
        .into_iter()
        .map(|definition| (definition.id, definition))
        .collect();
    let deck = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![DeckEntry {
            card: "RAV-CHAR".to_owned(),
            count: 5,
        }],
    };
    if deck
        .validate(
            &catalog,
            DeckRules {
                minimum_mainboard_size: 60,
                maximum_copies: 4,
                maximum_sideboard_size: 15,
            },
        )
        .is_ok()
    {
        return Some(Finding {
            code: "deck-validation-allows-sideboard-copy-limit-bypass",
            detail: "five nonbasic copies in the sideboard passed a four-copy deck rule".to_owned(),
        });
    }
    None
}

/// `Game::new` explicitly accepts three or more seats. Losing one player must
/// not end a three-player game while no winner exists.
fn probe_multiplayer_elimination() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 3).ok()?;
    game.players[2].life = 0;
    game.check_state_based_actions().ok()?;
    if game.players[2].lost && game.is_game_over() && game.winner().is_none() {
        return Some(Finding {
            code: "multiplayer-elimination-prematurely-ends-game",
            detail:
                "one eliminated player set is_game_over in a three-seat game with two survivors"
                    .to_owned(),
        });
    }
    None
}

/// In a three-seat game, an eliminated seat must be skipped by the priority
/// cycle rather than being asked to pass priority.
fn probe_eliminated_player_priority() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 3).ok()?;
    game.players[2].life = 0;
    game.check_state_based_actions().ok()?;
    game.pass_priority(PlayerId(0)).ok()?;
    game.pass_priority(PlayerId(1)).ok()?;
    let eliminated_pass = game.pass_priority(PlayerId(2));
    if eliminated_pass.is_ok() {
        return Some(Finding {
            code: "eliminated-player-receives-priority",
            detail: "priority advanced to a lost player, which then legally passed".to_owned(),
        });
    }
    None
}

/// The harder multiplayer case is an active player losing mid-turn. The engine
/// must still rotate the surviving players to the next turn and let that player
/// take a normal main-phase action without ever assigning priority to the loss.
fn probe_active_player_elimination_continues_multiplayer() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 3).ok()?;
    game.players[0].life = 0;
    game.add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
        .ok()?;
    game.add_card(PlayerId(2), "RAV-FOREST", Zone::Library)
        .ok()?;
    let land = game.add_card(PlayerId(1), "RAV-FOREST", Zone::Hand).ok()?;
    game.check_state_based_actions().ok()?;
    if game.is_game_over() || game.priority == PlayerId(0) {
        return Some(Finding {
            code: "active-player-elimination-does-not-normalize-priority",
            detail: format!(
                "game_over={} priority={:?} after active-player elimination",
                game.is_game_over(),
                game.priority
            ),
        });
    }
    for _ in 0..16 {
        if game.active_player == PlayerId(1) && game.step == Step::PrecombatMain {
            break;
        }
        let first = game.priority;
        game.pass_priority(first).ok()?;
        let second = game.priority;
        game.pass_priority(second).ok()?;
        if game.validate_invariants().is_err() {
            return Some(Finding {
                code: "active-player-elimination-breaks-continuing-invariants",
                detail: "a surviving multiplayer turn reached an invalid state".to_owned(),
            });
        }
    }
    if game.active_player != PlayerId(1) || game.step != Step::PrecombatMain {
        return Some(Finding {
            code: "active-player-elimination-does-not-advance-turn",
            detail: format!(
                "survivors did not reach player 1 main phase: {:?} {:?}",
                game.active_player, game.step
            ),
        });
    }
    if game.play_land(PlayerId(1), land).is_err() || game.validate_invariants().is_err() {
        return Some(Finding {
            code: "active-player-elimination-prevents-survivor-gameplay",
            detail: "the next surviving active player could not make a valid land play".to_owned(),
        });
    }
    None
}

/// Transmute is an activated ability restricted to sorcery timing. The public
/// operation must reject a response while another spell is on the stack.
fn probe_transmute_sorcery_timing() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    let char = game.add_card(PlayerId(0), "RAV-CHAR", Zone::Hand).ok()?;
    let muddle = game
        .add_card(PlayerId(1), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)
        .ok()?;
    let found = game
        .add_card(PlayerId(1), "RAV-LIGHTNING-HELIX", Zone::Library)
        .ok()?;
    game.grant_mana(PlayerId(0), Color::Red, 3).ok()?;
    game.grant_mana(PlayerId(1), Color::Blue, 3).ok()?;
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
        },
    )
    .ok()?;
    if !game.stack.is_empty() && game.transmute(PlayerId(1), muddle, found).is_ok() {
        return Some(Finding {
            code: "transmute-accepts-instant-speed-activation",
            detail: "Muddle the Mixture transmuted in response to a spell on the stack".to_owned(),
        });
    }
    None
}

/// Dredge is a replacement for a draw, not a free action from the graveyard.
/// `draw_card(_, Some(card))` is the controlled replacement path; this direct
/// call checks that the public API cannot create an out-of-window dredge event.
fn probe_out_of_window_dredge() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    let brownscale = game
        .add_card(PlayerId(0), "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)
        .ok()?;
    game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .ok()?;
    game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .ok()?;
    if game.dredge(PlayerId(0), brownscale).is_ok()
        && game
            .canonical_event_log()
            .iter()
            .any(|event| event.contains("Dredged"))
    {
        return Some(Finding {
            code: "dredge-accepts-out-of-window-activation",
            detail: "public dredge API moved cards without a pending draw replacement".to_owned(),
        });
    }
    None
}

/// A card with deliberately unsupported front-face text must surface a
/// capability gap, rather than resolve as a successful spell with no effect.
fn probe_unsupported_spell_front_face() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    let muddle = game
        .add_card(PlayerId(0), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)
        .ok()?;
    game.grant_mana(PlayerId(0), Color::Blue, 2).ok()?;
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: muddle,
            targets: Vec::new(),
            convoke: Vec::new(),
        },
    )
    .ok()?;
    game.pass_priority(PlayerId(1)).ok()?;
    game.pass_priority(PlayerId(0)).ok()?;
    if game.zone_of(muddle) == Some(Zone::Graveyard)
        && game
            .canonical_event_log()
            .iter()
            .any(|event| event.contains("SpellResolved"))
    {
        return Some(Finding {
            code: "unsupported-spell-front-face-resolves-as-noop",
            detail: "Muddle the Mixture cast successfully despite no supported cast effect"
                .to_owned(),
        });
    }
    None
}

/// Tokens cease to exist when they leave the battlefield. Combat bookkeeping may
/// still name the dead token until end of combat, so invariant validation must
/// tolerate that historical reference just as it does for nontoken creatures.
fn probe_combat_with_dead_token() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    let scatter = game
        .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
        .ok()?;
    let wurm = game
        .add_card(PlayerId(1), "RAV-SIEGE-WURM", Zone::Battlefield)
        .ok()?;
    game.add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
        .ok()?;
    game.grant_mana(PlayerId(0), Color::Green, 5).ok()?;
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: scatter,
            targets: Vec::new(),
            convoke: Vec::new(),
        },
    )
    .ok()?;
    game.pass_priority(PlayerId(1)).ok()?;
    game.pass_priority(PlayerId(0)).ok()?;
    let token = *game.players[0].battlefield.first()?;

    while game.step != Step::DeclareAttackers || game.active_player != PlayerId(1) {
        let first = game.priority;
        game.pass_priority(first).ok()?;
        let second = game.priority;
        game.pass_priority(second).ok()?;
    }
    game.declare_attackers(PlayerId(1), &[wurm]).ok()?;
    let first = game.priority;
    game.pass_priority(first).ok()?;
    let second = game.priority;
    game.pass_priority(second).ok()?;
    if game.step != Step::DeclareBlockers {
        return Some(Finding {
            code: "audit.setup-token-combat-failed",
            detail: format!("expected DeclareBlockers, got {:?}", game.step),
        });
    }
    game.declare_blockers(
        PlayerId(0),
        &[CombatBlock {
            attacker: wurm,
            blocker: token,
        }],
    )
    .ok()?;
    let first = game.priority;
    game.pass_priority(first).ok()?;
    let second = game.priority;
    game.pass_priority(second).ok()?;
    if game.zone_of(token).is_none() && game.validate_invariants().is_err() {
        return Some(Finding {
            code: "combat-state-breaks-after-token-dies",
            detail: "a dead token remained in combat bookkeeping and invalidated the game"
                .to_owned(),
        });
    }
    None
}
