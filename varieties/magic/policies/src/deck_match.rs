//! Deterministic, whole-deck development matches for the public RAV fixtures.
//!
//! This runner deliberately uses the normal public game setup and policy-submit
//! APIs: deck lists become libraries, libraries are shuffled, each player draws an
//! opening hand, then one policy proposal at a time goes through engine legality.
//! It is consequently useful both as an integration test and as a narrow detector
//! for engine behavior. Policy errors and development limits are recorded as match
//! terminations, never misrepresented as engine weaknesses. Only an invariant
//! failure or a policy's explicit `ReportEngineWeakness` action produces an engine
//! finding.

use cardbench_magic_engine::{Game, GameEvent, PlayerId, PolicyAction, PolicyMoveKind, RulesError};
use cardbench_magic_rav::{DeckFixture, card_definitions, event_digest, load_reference_decks};

use crate::{BorosTempoPolicy, CodePolicy, SelesnyaConvokePolicy};

/// Stable identifier for the public two-deck development match.
pub const RAV_DECK_MATCH_ID: &str = "rav_boros_vs_selesnya_full_deck";

/// Limits and deterministic setup for one full-deck policy development match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeckMatchConfig {
    /// Seed consumed in player order while the two libraries are shuffled.
    pub shuffle_seed: u64,
    /// Number of cards drawn by each player before the first turn.
    pub opening_hand_size: u8,
    /// Accepted policy moves allowed before the runner stops a bounded development run.
    pub max_policy_moves: u32,
    /// Turns allowed before the runner stops a bounded development run.
    pub max_turns: u32,
}

impl Default for DeckMatchConfig {
    fn default() -> Self {
        Self {
            shuffle_seed: 73,
            opening_hand_size: 7,
            max_policy_moves: 5_000,
            max_turns: 80,
        }
    }
}

/// The terminal reason observed by [`run_rav_full_deck_match`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeckMatchTermination {
    /// State-based actions left exactly one player in the game.
    Winner(PlayerId),
    /// The runner stopped a non-terminal game before another move could be accepted.
    MoveLimit { limit: u32 },
    /// The runner stopped a non-terminal game before another turn could start.
    TurnLimit { limit: u32 },
    /// A policy emitted an explicit capability-gap report through the engine.
    PolicyReportedWeakness {
        player: PlayerId,
        policy: String,
        code: String,
    },
    /// An otherwise valid game state rejected the policy's proposed action.
    PolicyMoveRejected {
        player: PlayerId,
        policy: String,
        kind: PolicyMoveKind,
        error: String,
    },
    /// The runner found an invariant violation after a setup or policy move.
    InvariantViolation { detail: String },
}

/// Classification deliberately reserved for evidence about the engine itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineFindingKind {
    /// The engine violated one of its own public state invariants.
    EngineBug,
    /// A policy explicitly encountered a rules capability the engine does not model.
    CapabilityGap,
}

/// A reproducible finding about engine behavior, separated from ordinary policy or
/// harness outcomes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineFinding {
    pub kind: EngineFindingKind,
    pub shuffle_seed: u64,
    pub player: Option<PlayerId>,
    pub code: String,
    pub detail: String,
}

/// Complete, deterministic record of a full-deck development match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckMatchResult {
    pub id: &'static str,
    pub config: DeckMatchConfig,
    pub termination: DeckMatchTermination,
    pub winner: Option<PlayerId>,
    pub life: [i16; 2],
    pub turns: u32,
    /// Number of policy proposals attempted by the match runner.
    pub attempted_policy_moves: u32,
    /// Number of proposals the engine accepted and recorded.
    pub accepted_policy_moves: u32,
    pub engine_findings: Vec<EngineFinding>,
    pub event_log: Vec<String>,
    pub digest: String,
}

impl DeckMatchResult {
    /// `true` only when a player won and no engine finding was reported.
    #[must_use]
    pub fn is_clean_completion(&self) -> bool {
        self.winner.is_some() && self.engine_findings.is_empty()
    }
}

/// Aggregate outcome of replaying the same public match across deterministic seeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckMatchSweepResult {
    pub id: &'static str,
    pub matches: Vec<DeckMatchResult>,
    pub engine_findings: Vec<EngineFinding>,
}

/// Runs the two public RAV reference decks from shuffled libraries until one player
/// loses or a deliberate development bound is reached.
///
/// Unlike the compact scripted policy match, this function is intentionally not
/// tied to a single fixed event digest. The catalog, policy, or rules slice may grow
/// while the public setup contract remains valid. Determinism is asserted by tests
/// through replaying the same configuration.
#[allow(clippy::too_many_lines)] // The explicit loop is the readable policy-to-engine audit trail.
pub fn run_rav_full_deck_match(config: DeckMatchConfig) -> Result<DeckMatchResult, String> {
    if config.opening_hand_size == 0 {
        return Err("full-deck match requires a nonzero opening hand size".to_owned());
    }
    if config.max_policy_moves == 0 || config.max_turns == 0 {
        return Err("full-deck match limits must both be nonzero".to_owned());
    }

    let decks = load_reference_decks().map_err(|error| error.to_string())?;
    let [boros_deck, selesnya_deck] = expected_decks(&decks)?;
    let mut game = Game::new(card_definitions(), 2).map_err(rules_error)?;
    game.set_shuffle_seed(config.shuffle_seed);
    game.load_deck_into_library(PlayerId(0), &boros_deck.deck)
        .map_err(rules_error)?;
    game.load_deck_into_library(PlayerId(1), &selesnya_deck.deck)
        .map_err(rules_error)?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(rules_error)?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(rules_error)?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        policy_for(PlayerId(0), &boros_deck.policy)?,
        policy_for(PlayerId(1), &selesnya_deck.policy)?,
    ];
    let mut attempted_policy_moves = 0;
    let mut accepted_policy_moves = 0;
    let mut engine_findings = Vec::new();

    if let Err(error) = game.validate_invariants() {
        engine_findings.push(invariant_finding(
            config.shuffle_seed,
            None,
            "after deck setup",
            &error,
        ));
        return Ok(result_from_game(
            &game,
            config,
            DeckMatchTermination::InvariantViolation {
                detail: format!("after deck setup: {error}"),
            },
            attempted_policy_moves,
            accepted_policy_moves,
            engine_findings,
        ));
    }

    let termination = loop {
        if game.is_game_over() {
            if let Some(winner) = game.winner() {
                break DeckMatchTermination::Winner(winner);
            }
            let detail = "game ended without exactly one surviving player".to_owned();
            engine_findings.push(EngineFinding {
                kind: EngineFindingKind::EngineBug,
                shuffle_seed: config.shuffle_seed,
                player: None,
                code: "game-over-without-single-winner".to_owned(),
                detail: detail.clone(),
            });
            break DeckMatchTermination::InvariantViolation { detail };
        }
        if game.turn > config.max_turns {
            break DeckMatchTermination::TurnLimit {
                limit: config.max_turns,
            };
        }
        if accepted_policy_moves >= config.max_policy_moves {
            break DeckMatchTermination::MoveLimit {
                limit: config.max_policy_moves,
            };
        }

        // This remains separate from priority during combat declarations, where
        // the next policy decision is a turn-based action.
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(rules_error)?;
        let policy = policies
            .get_mut(player.0)
            .ok_or_else(|| format!("no policy installed for seated player {}", player.0))?;
        let policy_id = policy.id().to_owned();
        let action = policy.propose_move(&view);
        let action_kind = action.kind();
        let reports_weakness = match &action {
            PolicyAction::ReportEngineWeakness { code, .. } => Some(code.clone()),
            _ => None,
        };
        attempted_policy_moves += 1;
        if let Err(error) = game.submit_policy_move(player, policy_id.clone(), action) {
            if let Err(invariant) = game.validate_invariants() {
                engine_findings.push(invariant_finding(
                    config.shuffle_seed,
                    Some(player),
                    "after rejected policy move",
                    &invariant,
                ));
                break DeckMatchTermination::InvariantViolation {
                    detail: format!(
                        "engine rejected {action_kind:?} from `{policy_id}` with `{error}` and then violated invariants: {invariant}"
                    ),
                };
            }
            break DeckMatchTermination::PolicyMoveRejected {
                player,
                policy: policy_id,
                kind: action_kind,
                error: error.to_string(),
            };
        }
        accepted_policy_moves += 1;
        if let Err(error) = game.validate_invariants() {
            engine_findings.push(invariant_finding(
                config.shuffle_seed,
                Some(player),
                &format!("after accepted {action_kind:?} from `{policy_id}`"),
                &error,
            ));
            break DeckMatchTermination::InvariantViolation {
                detail: format!("after accepted {action_kind:?} from `{policy_id}`: {error}"),
            };
        }
        if let Some(code) = reports_weakness {
            break DeckMatchTermination::PolicyReportedWeakness {
                player,
                policy: policy_id,
                code,
            };
        }
    };

    engine_findings.extend(collect_capability_findings(
        &game.event_log,
        config.shuffle_seed,
    ));
    Ok(result_from_game(
        &game,
        config,
        termination,
        attempted_policy_moves,
        accepted_policy_moves,
        engine_findings,
    ))
}

fn expected_decks(decks: &[DeckFixture]) -> Result<[&DeckFixture; 2], String> {
    let boros = decks
        .iter()
        .find(|deck| deck.id == "rav_boros_helix")
        .ok_or_else(|| "reference deck fixture `rav_boros_helix` is missing".to_owned())?;
    let selesnya = decks
        .iter()
        .find(|deck| deck.id == "rav_selesnya_convoke")
        .ok_or_else(|| "reference deck fixture `rav_selesnya_convoke` is missing".to_owned())?;
    Ok([boros, selesnya])
}

fn policy_for(player: PlayerId, id: &str) -> Result<Box<dyn CodePolicy>, String> {
    match id {
        "rav.boros-tempo.v1" => Ok(Box::new(BorosTempoPolicy::new(player))),
        "rav.selesnya-convoke.v1" => Ok(Box::new(SelesnyaConvokePolicy::new(player))),
        _ => Err(format!("public deck specifies unknown Rust policy `{id}`")),
    }
}

/// Replays the public full-deck match under each supplied deterministic shuffle
/// seed. It is intentionally a direct aggregation: ordinary policy rejections and
/// bounds remain on their individual match results, while only engine findings are
/// promoted to the sweep summary.
pub fn run_rav_full_deck_sweep(
    seeds: impl IntoIterator<Item = u64>,
) -> Result<DeckMatchSweepResult, String> {
    let mut matches = Vec::new();
    let mut engine_findings = Vec::new();
    for shuffle_seed in seeds {
        let result = run_rav_full_deck_match(DeckMatchConfig {
            shuffle_seed,
            ..DeckMatchConfig::default()
        })?;
        engine_findings.extend(result.engine_findings.iter().cloned());
        matches.push(result);
    }
    if matches.is_empty() {
        return Err("full-deck sweep requires at least one shuffle seed".to_owned());
    }
    Ok(DeckMatchSweepResult {
        id: "rav_boros_vs_selesnya_full_deck_sweep",
        matches,
        engine_findings,
    })
}

fn result_from_game(
    game: &Game,
    config: DeckMatchConfig,
    termination: DeckMatchTermination,
    attempted_policy_moves: u32,
    accepted_policy_moves: u32,
    engine_findings: Vec<EngineFinding>,
) -> DeckMatchResult {
    let event_log = game.canonical_event_log();
    DeckMatchResult {
        id: RAV_DECK_MATCH_ID,
        config,
        termination,
        winner: game.winner(),
        life: [game.players[0].life, game.players[1].life],
        turns: game.turn,
        attempted_policy_moves,
        accepted_policy_moves,
        engine_findings,
        digest: event_digest(&event_log),
        event_log,
    }
}

fn invariant_finding(
    shuffle_seed: u64,
    player: Option<PlayerId>,
    context: &str,
    error: &RulesError,
) -> EngineFinding {
    EngineFinding {
        kind: EngineFindingKind::EngineBug,
        shuffle_seed,
        player,
        code: "engine-invariant-violation".to_owned(),
        detail: format!("{context}: {error}"),
    }
}

fn collect_capability_findings(events: &[GameEvent], shuffle_seed: u64) -> Vec<EngineFinding> {
    events
        .iter()
        .filter_map(|event| match event {
            GameEvent::EngineWeaknessRevealed {
                player,
                code,
                detail,
            } => Some(EngineFinding {
                kind: EngineFindingKind::CapabilityGap,
                shuffle_seed,
                player: Some(*player),
                code: code.clone(),
                detail: detail.clone(),
            }),
            _ => None,
        })
        .collect()
}

#[allow(clippy::needless_pass_by_value)] // `Result::map_err` provides an owned error.
fn rules_error(error: RulesError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_deck_match_is_deterministic_and_records_setup_events() {
        let first = run_rav_full_deck_match(DeckMatchConfig::default())
            .expect("full deck development match");
        let second = run_rav_full_deck_match(DeckMatchConfig::default())
            .expect("deterministic full deck replay");
        assert_eq!(first, second);
        assert!(first.accepted_policy_moves > 0);
        assert!(first.is_clean_completion());
        assert_eq!(first.termination, DeckMatchTermination::Winner(PlayerId(1)));
        assert_eq!(first.winner, Some(PlayerId(1)));
        assert_eq!(first.turns, 30);
        assert_eq!(first.accepted_policy_moves, 957);
        assert_eq!(first.life, [-2, 10]);
        assert_eq!(first.digest, "fnv1a64:d578c3d3df34e81e");
        for marker in [
            "DeckLoaded",
            "LibraryShuffled",
            "OpeningHandDrawn",
            "ManaAbilityActivated",
            "SpellCast",
            "AttackersDeclared",
            "BlockersDeclared",
            "DamageDealtToPlayer",
            "PlayerLost",
        ] {
            assert!(
                first.event_log.iter().any(|event| event.contains(marker)),
                "default full-deck trace is missing `{marker}`"
            );
        }
    }

    #[test]
    fn bounded_nonterminal_match_is_not_misclassified_as_an_engine_finding() {
        let result = run_rav_full_deck_match(DeckMatchConfig {
            max_policy_moves: 1,
            ..DeckMatchConfig::default()
        })
        .expect("bounded full deck development match");
        assert_eq!(
            result.termination,
            DeckMatchTermination::MoveLimit { limit: 1 }
        );
        assert_eq!(result.accepted_policy_moves, 1);
        assert!(result.engine_findings.is_empty());
        assert!(!result.is_clean_completion());
    }

    #[test]
    fn multi_seed_sweep_aggregates_only_actual_engine_findings() {
        let sweep = run_rav_full_deck_sweep([73, 74]).expect("two-seed full deck sweep");
        assert_eq!(sweep.matches.len(), 2);
        assert!(sweep.engine_findings.is_empty());
    }
}
