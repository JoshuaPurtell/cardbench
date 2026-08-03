//! Adversarial public-API probes for engine bugs.
//!
//! This is intentionally not a policy tournament. Each probe asks whether a
//! core rule boundary fails loudly and leaves a valid state behind.

use std::io::{self, Write};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CastRequest, Color, CombatBlock, DeckEntry, DeckList, DeckRules, Game, GameEvent, PlayerId,
    PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{
    EngineTournamentFailure, run_rav_reference_deck_matrix, run_rav_trigger_probe,
};
use cardbench_magic_rav::{card_definitions, load_reference_decks};

#[derive(Debug)]
struct Finding {
    code: &'static str,
    detail: String,
}

// Keep the always-on audit within an interactive development cycle. The
// dedicated `rav-reference-deck-matrix` binary runs the wider eight-seed sweep.
const DEFAULT_POLICY_MATRIX_SEED_COUNT: u64 = 1;

#[derive(Debug)]
struct PolicyMatrixProbe {
    deck_count: usize,
    match_count: usize,
    findings: Vec<Finding>,
}

fn main() -> ExitCode {
    let quick = std::env::args()
        .skip(1)
        .any(|argument| argument == "--quick");
    println!("schema_version=cardbench.magic.engine-audit.v1");
    println!("audit_mode={}", if quick { "quick" } else { "campaign" });
    println!("audit_progress=public-api-probes state=started");
    flush_stdout();
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
        probe_muddle_counterspell_resolution(),
        probe_combat_with_dead_token(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    println!("audit_progress=public-api-probes state=completed");
    flush_stdout();
    if quick {
        println!("policy_matrix=skipped reason=quick-mode");
    } else {
        let policy_matrix_seed_count = configured_policy_matrix_seed_count();
        let policy_matrix = probe_policy_matchup_matrix(policy_matrix_seed_count);
        findings.extend(policy_matrix.findings);
        println!("policy_matrix_deck_count={}", policy_matrix.deck_count);
        println!("policy_matrix_seed_count={policy_matrix_seed_count}");
        println!("policy_matrix_match_count={}", policy_matrix.match_count);
    }
    let trigger_probe = run_rav_trigger_probe();
    match trigger_probe {
        Ok(result) if result.passed() => {
            println!(
                "trigger_probe_id={} triggered_ability_count={} event_digest={} passed=true",
                result.id, result.triggered_ability_count, result.digest
            );
        }
        Ok(result) => {
            println!(
                "trigger_probe_id={} triggered_ability_count={} event_digest={} passed=false",
                result.id, result.triggered_ability_count, result.digest
            );
            findings.push(Finding {
                code: "trigger-probe-failed",
                detail: format!(
                    "{} produced {} triggered abilities",
                    result.id, result.triggered_ability_count
                ),
            });
        }
        Err(error) => {
            println!("trigger_probe=failed detail={error}");
            findings.push(Finding {
                code: "trigger-probe-setup-failed",
                detail: error,
            });
        }
    }
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

fn configured_policy_matrix_seed_count() -> u64 {
    std::env::var("RAV_AUDIT_SEED_COUNT")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|count| *count > 0)
        .unwrap_or(DEFAULT_POLICY_MATRIX_SEED_COUNT)
}

fn flush_stdout() {
    io::stdout().flush().expect("audit progress must flush");
}

/// Runs every ordered pair of public reference decks. It exercises actual
/// policy submissions and turns every engine finding, rejection, capability
/// gap, or bounded incomplete run into an audit failure with deck provenance.
fn probe_policy_matchup_matrix(seed_count: u64) -> PolicyMatrixProbe {
    let deck_count = load_reference_decks().map_or(0, |decks| decks.len());
    let mut match_count = 0;
    let mut findings = Vec::new();
    for seed in 0..seed_count {
        println!("audit_progress=policy-matrix seed={seed} state=started");
        flush_stdout();
        match run_rav_reference_deck_matrix([seed]) {
            Ok(matrix) => {
                let seed_match_count = matrix.matches.len();
                let mut seed_findings = matrix
                    .failures
                    .into_iter()
                    .map(|failure| match failure {
                        EngineTournamentFailure::EngineFinding(finding) => Finding {
                            code: "policy-matrix-engine-finding",
                            detail: format!("{finding:?}"),
                        },
                        EngineTournamentFailure::IncompleteTermination {
                            deck_ids,
                            shuffle_seed,
                            termination,
                        } => Finding {
                            code: "policy-matrix-incomplete-run",
                            detail: format!(
                                "{}-vs-{}-seed-{shuffle_seed}: {termination:?}",
                                deck_ids[0], deck_ids[1]
                            ),
                        },
                    })
                    .collect::<Vec<_>>();
                println!(
                    "audit_progress=policy-matrix seed={seed} state=completed match_count={seed_match_count} finding_count={}",
                    seed_findings.len()
                );
                flush_stdout();
                match_count += seed_match_count;
                findings.append(&mut seed_findings);
            }
            Err(error) => {
                println!("audit_progress=policy-matrix seed={seed} state=failed");
                flush_stdout();
                findings.push(Finding {
                    code: "policy-matrix-setup-failed",
                    detail: error,
                });
                break;
            }
        }
    }
    PolicyMatrixProbe {
        deck_count,
        match_count,
        findings,
    }
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
            payment_mana_abilities: vec![],
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
            payment_mana_abilities: vec![],
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
    let _found = game
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
            payment_mana_abilities: vec![],
        },
    )
    .ok()?;
    if !game.stack.is_empty() && game.activate_transmute(PlayerId(1), muddle).is_ok() {
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

/// Muddle's executable counter face must remove the targeted instant or
/// sorcery from the stack, produce an explicit counter event, and prevent the
/// target's effects from resolving. This is distinct from the all-targets-
/// illegal rules-counter event.
fn probe_muddle_counterspell_resolution() -> Option<Finding> {
    let mut game = Game::new(card_definitions(), 2).ok()?;
    let char = game.add_card(PlayerId(0), "RAV-CHAR", Zone::Hand).ok()?;
    let muddle = game
        .add_card(PlayerId(1), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)
        .ok()?;
    game.grant_mana(PlayerId(0), Color::Red, 3).ok()?;
    game.grant_mana(PlayerId(1), Color::Blue, 2).ok()?;
    if game
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: char,
                targets: vec![Target::Player(PlayerId(1))],
                convoke: Vec::new(),
                payment_mana_abilities: vec![],
            },
        )
        .is_err()
    {
        return Some(Finding {
            code: "muddle-counterspell-setup-failed",
            detail: "the prepared Char spell could not enter the stack".to_owned(),
        });
    }
    if game.pass_priority(PlayerId(0)).is_err() {
        return Some(Finding {
            code: "muddle-counterspell-response-window-failed",
            detail: "the Char caster could not pass priority to open a response window".to_owned(),
        });
    }
    if game
        .cast_spell(
            PlayerId(1),
            CastRequest {
                card: muddle,
                targets: vec![Target::Spell(char)],
                convoke: Vec::new(),
                payment_mana_abilities: vec![],
            },
        )
        .is_err()
    {
        return Some(Finding {
            code: "muddle-counterspell-cannot-target-stack-spell",
            detail: "Muddle rejected an opposing instant currently on the stack".to_owned(),
        });
    }
    let first = game.priority;
    if game.pass_priority(first).is_err() {
        return Some(Finding {
            code: "muddle-counterspell-resolution-pass-failed",
            detail: "the first post-response priority pass was rejected".to_owned(),
        });
    }
    let second = game.priority;
    if game.pass_priority(second).is_err() {
        return Some(Finding {
            code: "muddle-counterspell-resolution-pass-failed",
            detail: "the second post-response priority pass was rejected".to_owned(),
        });
    }
    if game.zone_of(char) != Some(Zone::Graveyard)
        || game.zone_of(muddle) != Some(Zone::Graveyard)
        || !game.stack.is_empty()
        || game.players[1].life != 20
        || !game.event_log.iter().any(|event| {
            matches!(
                event,
                GameEvent::SpellCountered { card, source } if *card == char && *source == muddle
            )
        })
        || game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == char))
        || game.validate_invariants().is_err()
    {
        return Some(Finding {
            code: "muddle-counterspell-resolution-invalid",
            detail: "Muddle did not counter Char as an explicit stack interaction".to_owned(),
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
    game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
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
            payment_mana_abilities: vec![],
        },
    )
    .ok()?;
    game.pass_priority(PlayerId(1)).ok()?;
    game.pass_priority(PlayerId(0)).ok()?;
    let token = *game.players[0].battlefield.first()?;

    while game.step != Step::DeclareAttackers || game.active_player != PlayerId(0) {
        let first = game.priority;
        game.pass_priority(first).ok()?;
        let second = game.priority;
        game.pass_priority(second).ok()?;
    }
    game.declare_attackers(PlayerId(0), &[token]).ok()?;
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
        PlayerId(1),
        &[CombatBlock {
            attacker: token,
            blocker: wurm,
        }],
    )
    .ok()?;
    let first = game.priority;
    game.pass_priority(first).ok()?;
    let second = game.priority;
    game.pass_priority(second).ok()?;
    if game.zone_of(token).is_none() && game.view_for_player(PlayerId(0)).is_err() {
        return Some(Finding {
            code: "combat-view-breaks-after-token-dies",
            detail: "a dead token in historical combat state made GameView construction fail"
                .to_owned(),
        });
    }
    if game.zone_of(token).is_none() && game.validate_invariants().is_err() {
        return Some(Finding {
            code: "combat-state-breaks-after-token-dies",
            detail: "a dead token remained in combat bookkeeping and invalidated the game"
                .to_owned(),
        });
    }
    None
}
