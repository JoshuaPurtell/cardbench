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

use crate::{
    BorosCharControlPolicy, BorosConvokeBurnPolicy, BorosRadianceAssaultPolicy, BorosTempoPolicy,
    BorosTokenRallyPolicy, CodePolicy, DimirTransmuteAttritionPolicy, DimirTransmuteConvokePolicy,
    DimirTransmuteHelixPolicy, GolgariAttritionPolicy, GolgariDredgeGrindPolicy,
    GolgariWurmPressPolicy, RadianceConvokeAssaultPolicy, SelesnyaConvokePolicy,
    SelesnyaRadianceTokensPolicy, SelesnyaSiegePolicy,
};

/// Stable identifier for the public two-deck development match.
pub const RAV_DECK_MATCH_ID: &str = "rav_boros_vs_selesnya_full_deck";
/// Stable identifier for the all-reference-deck, ordered policy matrix.
pub const RAV_REFERENCE_DECK_MATRIX_ID: &str = "rav_reference_deck_policy_matrix";

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
            // A sixty-card two-player game can legitimately reach turn 108
            // before a player loses to an empty library. Leave room for that
            // fail-closed result instead of mistaking it for a policy stall.
            max_turns: 120,
        }
    }
}

/// The terminal reason observed by [`run_rav_full_deck_match`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeckMatchTermination {
    /// State-based actions left exactly one player in the game.
    Winner(PlayerId),
    /// State-based actions removed every player simultaneously. This is a
    /// valid completed draw, not an engine or policy failure.
    Draw,
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
    /// Public fixtures seated as player zero and player one for this trace.
    /// This keeps generic matchup evidence attributable even though the legacy
    /// match identifier remains stable for existing consumers.
    pub deck_ids: [String; 2],
    pub config: DeckMatchConfig,
    pub termination: DeckMatchTermination,
    pub winner: Option<PlayerId>,
    pub life: [i64; 2],
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

/// A fail-closed public engine probe. Unlike a convenience sweep, it rejects any
/// invariant finding, capability gap, policy rejection, or bounded incomplete
/// run. A rules-valid draw is a completed game.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineTournamentResult {
    pub id: &'static str,
    pub matches: Vec<DeckMatchResult>,
    pub failures: Vec<EngineTournamentFailure>,
}

impl EngineTournamentResult {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// An incomplete full-deck probe, deliberately retaining policy failures
/// separately from actual engine findings so triage is honest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineTournamentFailure {
    EngineFinding(EngineFinding),
    IncompleteTermination {
        deck_ids: [String; 2],
        shuffle_seed: u64,
        termination: DeckMatchTermination,
    },
}

/// Runs the two public RAV reference decks from shuffled libraries until one player
/// loses or a deliberate development bound is reached.
///
/// Unlike the compact scripted policy match, this function is intentionally not
/// tied to a single fixed event digest. The catalog, policy, or rules slice may grow
/// while the public setup contract remains valid. Determinism is asserted by tests
/// through replaying the same configuration.
pub fn run_rav_full_deck_match(config: DeckMatchConfig) -> Result<DeckMatchResult, String> {
    run_rav_deck_matchup(config, "rav_boros_helix", "rav_selesnya_convoke")
}

/// Runs any two shown RAV deck fixtures against their declared Rust policies.
/// Both deck ids must be present in the public reference-deck index.
#[allow(clippy::too_many_lines)] // The explicit loop is the readable policy-to-engine audit trail.
pub fn run_rav_deck_matchup(
    config: DeckMatchConfig,
    deck_p0_id: &str,
    deck_p1_id: &str,
) -> Result<DeckMatchResult, String> {
    if config.opening_hand_size == 0 {
        return Err("full-deck match requires a nonzero opening hand size".to_owned());
    }
    if config.max_policy_moves == 0 || config.max_turns == 0 {
        return Err("full-deck match limits must both be nonzero".to_owned());
    }

    let decks = load_reference_decks().map_err(|error| error.to_string())?;
    let deck_p0 = expected_deck(&decks, deck_p0_id)?;
    let deck_p1 = expected_deck(&decks, deck_p1_id)?;
    let deck_ids = [deck_p0.id.clone(), deck_p1.id.clone()];
    let mut game = Game::new(card_definitions(), 2).map_err(rules_error)?;
    game.set_shuffle_seed(config.shuffle_seed);
    game.load_deck_into_library(PlayerId(0), &deck_p0.deck)
        .map_err(rules_error)?;
    game.load_deck_into_library(PlayerId(1), &deck_p1.deck)
        .map_err(rules_error)?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(rules_error)?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(rules_error)?;
    game.begin_game().map_err(rules_error)?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        policy_for(PlayerId(0), &deck_p0.policy)?,
        policy_for(PlayerId(1), &deck_p1.policy)?,
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
            deck_ids,
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
            break DeckMatchTermination::Draw;
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
        let action = if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else {
            policy.propose_move(&view)
        };
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
        deck_ids,
        config,
        termination,
        attempted_policy_moves,
        accepted_policy_moves,
        engine_findings,
    ))
}

fn expected_deck<'a>(decks: &'a [DeckFixture], id: &str) -> Result<&'a DeckFixture, String> {
    decks
        .iter()
        .find(|deck| deck.id == id)
        .ok_or_else(|| format!("reference deck fixture `{id}` is missing"))
}

fn policy_for(player: PlayerId, id: &str) -> Result<Box<dyn CodePolicy>, String> {
    match id {
        "rav.boros-tempo.v1" => Ok(Box::new(BorosTempoPolicy::new(player))),
        "rav.boros-char-control.v1" => Ok(Box::new(BorosCharControlPolicy::new(player))),
        "rav.boros-convoke-burn.v1" => Ok(Box::new(BorosConvokeBurnPolicy::new(player))),
        "rav.boros-radiance-assault.v1" => Ok(Box::new(BorosRadianceAssaultPolicy::new(player))),
        "rav.boros-token-rally.v1" => Ok(Box::new(BorosTokenRallyPolicy::new(player))),
        "rav.dimir-transmute-attrition.v1" => {
            Ok(Box::new(DimirTransmuteAttritionPolicy::new(player)))
        }
        "rav.dimir-transmute-convoke.v1" => Ok(Box::new(DimirTransmuteConvokePolicy::new(player))),
        "rav.dimir-transmute-helix.v1" => Ok(Box::new(DimirTransmuteHelixPolicy::new(player))),
        "rav.golgari-attrition.v1" => Ok(Box::new(GolgariAttritionPolicy::new(player))),
        "rav.golgari-dredge-grind.v1" => Ok(Box::new(GolgariDredgeGrindPolicy::new(player))),
        "rav.golgari-wurm-press.v1" => Ok(Box::new(GolgariWurmPressPolicy::new(player))),
        "rav.radiance-convoke-assault.v1" => {
            Ok(Box::new(RadianceConvokeAssaultPolicy::new(player)))
        }
        "rav.selesnya-convoke.v1" => Ok(Box::new(SelesnyaConvokePolicy::new(player))),
        "rav.selesnya-radiance-tokens.v1" => {
            Ok(Box::new(SelesnyaRadianceTokensPolicy::new(player)))
        }
        "rav.selesnya-siege.v1" => Ok(Box::new(SelesnyaSiegePolicy::new(player))),
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

/// Runs a fail-closed multi-seed engine probe. An invariant violation is an
/// `EngineBug`; an explicit weakness report is a `CapabilityGap`; and a policy
/// rejection or runner bound fails the probe without being falsely classified as
/// an engine defect.
pub fn run_rav_engine_tournament(
    seeds: impl IntoIterator<Item = u64>,
) -> Result<EngineTournamentResult, String> {
    let sweep = run_rav_full_deck_sweep(seeds)?;
    Ok(fail_closed_tournament(
        "rav_boros_vs_selesnya_engine_tournament",
        sweep.matches,
        sweep.engine_findings,
    ))
}

/// Runs every ordered pair of public reference decks across each supplied
/// shuffle seed. The result fails closed on every engine finding, capability
/// report, rejected policy action, or bounded incomplete run. A rules-valid
/// draw is a normal completed result.
pub fn run_rav_reference_deck_matrix(
    seeds: impl IntoIterator<Item = u64>,
) -> Result<EngineTournamentResult, String> {
    let seeds: Vec<_> = seeds.into_iter().collect();
    if seeds.is_empty() {
        return Err("reference deck matrix requires at least one shuffle seed".to_owned());
    }
    let deck_ids: Vec<_> = load_reference_decks()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|deck| deck.id)
        .collect();
    if deck_ids.len() < 2 {
        return Err("reference deck matrix requires at least two decks".to_owned());
    }
    let mut matches = Vec::new();
    let mut engine_findings = Vec::new();
    for deck_p0 in &deck_ids {
        for deck_p1 in &deck_ids {
            if deck_p0 == deck_p1 {
                continue;
            }
            for &shuffle_seed in &seeds {
                let result = run_rav_deck_matchup(
                    DeckMatchConfig {
                        shuffle_seed,
                        ..DeckMatchConfig::default()
                    },
                    deck_p0,
                    deck_p1,
                )
                .map_err(|error| {
                    format!(
                        "reference matrix {deck_p0} vs {deck_p1} at seed {shuffle_seed}: {error}"
                    )
                })?;
                engine_findings.extend(result.engine_findings.iter().cloned());
                matches.push(result);
            }
        }
    }
    Ok(fail_closed_tournament(
        RAV_REFERENCE_DECK_MATRIX_ID,
        matches,
        engine_findings,
    ))
}

fn fail_closed_tournament(
    id: &'static str,
    matches: Vec<DeckMatchResult>,
    engine_findings: Vec<EngineFinding>,
) -> EngineTournamentResult {
    let mut failures: Vec<_> = engine_findings
        .into_iter()
        .map(EngineTournamentFailure::EngineFinding)
        .collect();
    for result in &matches {
        if !matches!(
            result.termination,
            DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
        ) {
            failures.push(EngineTournamentFailure::IncompleteTermination {
                deck_ids: result.deck_ids.clone(),
                shuffle_seed: result.config.shuffle_seed,
                termination: result.termination.clone(),
            });
        }
    }
    EngineTournamentResult {
        id,
        matches,
        failures,
    }
}

fn result_from_game(
    game: &Game,
    deck_ids: [String; 2],
    config: DeckMatchConfig,
    termination: DeckMatchTermination,
    attempted_policy_moves: u32,
    accepted_policy_moves: u32,
    engine_findings: Vec<EngineFinding>,
) -> DeckMatchResult {
    let event_log = game.canonical_event_log();
    DeckMatchResult {
        id: RAV_DECK_MATCH_ID,
        deck_ids,
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
        assert_eq!(first.accepted_policy_moves, 759);
        assert_eq!(first.life, [-2, 10]);
        assert_eq!(first.digest, "fnv1a64:9c899905a0c09b87");
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

    #[test]
    fn added_char_and_siege_decks_complete_a_real_shuffled_match() {
        let first = run_rav_deck_matchup(
            DeckMatchConfig::default(),
            "rav_boros_char_control",
            "rav_selesnya_siege",
        )
        .expect("Char versus Siege match");
        let second = run_rav_deck_matchup(
            DeckMatchConfig::default(),
            "rav_boros_char_control",
            "rav_selesnya_siege",
        )
        .expect("deterministic Char versus Siege replay");
        assert_eq!(first, second);
        assert!(first.is_clean_completion(), "{first:#?}");
        assert!(matches!(first.termination, DeckMatchTermination::Winner(_)));
        for marker in [
            "DeckLoaded",
            "ManaAbilityActivated",
            "SpellCast",
            "AttackersDeclared",
            "BlockersDeclared",
            "DamageDealtToPlayer",
            "PlayerLost",
        ] {
            assert!(
                first.event_log.iter().any(|event| event.contains(marker)),
                "Char versus Siege trace is missing `{marker}`"
            );
        }
    }

    #[test]
    fn sixteen_seed_tournament_fails_closed_on_all_engine_or_policy_errors() {
        let tournament = run_rav_engine_tournament(0..16).expect("sixteen-seed engine probe");
        assert_eq!(tournament.matches.len(), 16);
        assert!(tournament.passed(), "{tournament:#?}");
    }

    #[test]
    fn every_reference_deck_pair_has_an_attributable_fail_closed_trace() {
        let deck_count = load_reference_decks()
            .expect("public reference decks")
            .len();
        let matrix = run_rav_reference_deck_matrix([0]).expect("one-seed reference matrix");
        assert_eq!(matrix.id, RAV_REFERENCE_DECK_MATRIX_ID);
        assert_eq!(matrix.matches.len(), deck_count * (deck_count - 1));
        assert!(matrix.passed(), "{matrix:#?}");
        assert!(matrix.matches.iter().all(|match_result| {
            match_result.deck_ids[0] != match_result.deck_ids[1]
                && matches!(
                    match_result.termination,
                    DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
                )
        }));
    }

    #[test]
    fn fail_closed_tournament_accepts_a_rules_valid_simultaneous_loss_draw() {
        let draw = DeckMatchResult {
            id: RAV_DECK_MATCH_ID,
            deck_ids: ["fixture-a".to_owned(), "fixture-b".to_owned()],
            config: DeckMatchConfig::default(),
            termination: DeckMatchTermination::Draw,
            winner: None,
            life: [0, 0],
            turns: 1,
            attempted_policy_moves: 0,
            accepted_policy_moves: 0,
            engine_findings: vec![],
            event_log: vec!["GameEnded { winner: None }".to_owned()],
            digest: "fixture".to_owned(),
        };
        let tournament = fail_closed_tournament("draw-regression", vec![draw], vec![]);
        assert!(tournament.passed(), "{tournament:#?}");
    }
}
