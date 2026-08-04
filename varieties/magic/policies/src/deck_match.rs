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
use cardbench_magic_rav::{
    DeckFixture, RAV_CATALOG_COVERAGE_DECK_COUNT, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, event_digest,
    load_catalog_coverage_decks, load_constructed_decks, load_reference_decks, new_rav_game,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings, rav_land_entry_bindings,
    rav_mana_ability_bindings,
};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use crate::{
    Archetype, BorosCharControlPolicy, BorosConvokeBurnPolicy, BorosRadianceAssaultPolicy,
    BorosTempoPolicy, BorosTokenRallyPolicy, CatalogPolicyProfile, CodePolicy,
    DimirTransmuteAttritionPolicy, DimirTransmuteConvokePolicy, DimirTransmuteHelixPolicy,
    GolgariAttritionPolicy, GolgariDredgeGrindPolicy, GolgariWurmPressPolicy,
    RadianceConvokeAssaultPolicy, RavCatalogPolicy, SelesnyaConvokePolicy,
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
    /// `true` only when the game reached a rules-valid terminal state and no
    /// engine finding was reported. A simultaneous-loss draw is terminal and
    /// therefore clean just like a winner.
    #[must_use]
    pub fn is_clean_completion(&self) -> bool {
        matches!(
            self.termination,
            DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
        ) && self.engine_findings.is_empty()
    }
}

/// Aggregate outcome of replaying the same public match across deterministic seeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckMatchSweepResult {
    pub id: &'static str,
    pub matches: Vec<DeckMatchResult>,
    pub engine_findings: Vec<EngineFinding>,
}

impl DeckMatchSweepResult {
    /// The public sweep runner is fail-closed when consumed as a campaign:
    /// every seed must terminate with a winner or rules-valid draw and no
    /// invariant/capability finding may be present.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.engine_findings.is_empty()
            && self
                .matches
                .iter()
                .all(DeckMatchResult::is_clean_completion)
    }
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

/// Fail-closed result for running every generated catalog band against a
/// stable interactive reference opponent. Each match retains its canonical
/// event log and digest through [`DeckMatchResult`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogGauntletResult {
    pub deck_count: usize,
    pub policy_profile_count: usize,
    pub covered_definition_count: usize,
    pub covered_printing_count: usize,
    pub tournament: EngineTournamentResult,
}

impl CatalogGauntletResult {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.deck_count == RAV_CATALOG_COVERAGE_DECK_COUNT
            && self.policy_profile_count == 6
            && self.covered_definition_count == RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
            && self.covered_printing_count == RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
            && self.tournament.passed()
    }
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
    run_rav_deck_matchup_verified(config, "rav_boros_helix", "rav_selesnya_convoke")
}

/// Runs any two shown RAV deck fixtures against their declared Rust policies.
/// Both deck ids must be present in the public reference-deck index.
#[allow(clippy::too_many_lines)] // The explicit loop is the readable policy-to-engine audit trail.
pub fn run_rav_deck_matchup(
    config: DeckMatchConfig,
    deck_p0_id: &str,
    deck_p1_id: &str,
) -> Result<DeckMatchResult, String> {
    let decks = all_deck_fixtures()?;
    let pilots: [Box<dyn CodePolicy>; 2] = [
        pilot_for(PlayerId(0), expected_deck(&decks, deck_p0_id)?)?,
        pilot_for(PlayerId(1), expected_deck(&decks, deck_p1_id)?)?,
    ];
    run_deck_matchup_with(config, deck_p0_id, deck_p1_id, pilots)
}

/// Runs one full-deck match with explicitly supplied pilots.
///
/// Separated from [`run_rav_deck_matchup`] so the ladder can seat two different
/// policy generations on the same deck list; every other detail of setup,
/// legality, and receipting is shared, which is what makes the two paths
/// comparable.
///
/// # Errors
///
/// Returns an error when the configuration is degenerate, a deck id is unknown,
/// or game setup fails.
pub fn run_deck_matchup_with(
    config: DeckMatchConfig,
    deck_p0_id: &str,
    deck_p1_id: &str,
    pilots: [Box<dyn CodePolicy>; 2],
) -> Result<DeckMatchResult, String> {
    run_deck_matchup_capturing(config, deck_p0_id, deck_p1_id, pilots).map(|(result, ..)| result)
}

/// Runs one full-deck match and also returns the typed engine event log.
///
/// The ordinary runner keeps only canonical strings, which is all a campaign
/// needs and is cheaper across thousands of games. A reviewer needs the typed
/// events so a transcript can carry queryable fields rather than text a
/// consumer has to parse back.
///
/// # Errors
///
/// Returns an error when the configuration is degenerate, a deck id is unknown,
/// or game setup fails.
#[allow(clippy::too_many_lines)] // One ordered match loop stays more reviewable than a split one.
pub fn run_deck_matchup_capturing(
    config: DeckMatchConfig,
    deck_p0_id: &str,
    deck_p1_id: &str,
    pilots: [Box<dyn CodePolicy>; 2],
) -> Result<(DeckMatchResult, Vec<GameEvent>, Vec<ObjectIdentity>), String> {
    if config.opening_hand_size == 0 {
        return Err("full-deck match requires a nonzero opening hand size".to_owned());
    }
    if config.max_policy_moves == 0 || config.max_turns == 0 {
        return Err("full-deck match limits must both be nonzero".to_owned());
    }

    let decks = all_deck_fixtures()?;
    let deck_p0 = expected_deck(&decks, deck_p0_id)?;
    let deck_p1 = expected_deck(&decks, deck_p1_id)?;
    let deck_ids = [deck_p0.id.clone(), deck_p1.id.clone()];
    let mut game = new_rav_game(2).map_err(rules_error)?;
    game.set_shuffle_seed(config.shuffle_seed)
        .map_err(rules_error)?;
    game.load_deck_into_library(PlayerId(0), &deck_p0.deck)
        .map_err(rules_error)?;
    game.load_deck_into_library(PlayerId(1), &deck_p1.deck)
        .map_err(rules_error)?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(rules_error)?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(rules_error)?;
    game.begin_game().map_err(rules_error)?;
    // Snapshot identities now, while every card still exists. The end-of-game
    // pass below adds tokens created during play.
    let mut identities = BTreeMap::new();
    object_identities(&game, &mut identities);
    let mut policies = pilots;
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
        object_identities(&game, &mut identities);
        return Ok(captured_with(
            identities,
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
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
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
        // The dispatched action's atomic transaction runs the complete
        // invariant suite before submit_policy_move appends its policy
        // receipt. The outer receipt boundary performs its own cheap sealed
        // state/event audit; repeating the full event-history scan here made
        // long policy campaigns unnecessarily quadratic.
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
    object_identities(&game, &mut identities);
    Ok(captured_with(
        identities,
        &game,
        deck_ids,
        config,
        termination,
        attempted_policy_moves,
        accepted_policy_moves,
        engine_findings,
    ))
}

/// Executes one public matchup twice from fresh state and fails closed if the
/// canonical event transcript or digest drifts. Campaign binaries use this
/// boundary so a policy or engine that depends on hidden iteration order cannot
/// silently produce a different result on replay.
fn run_rav_deck_matchup_verified(
    config: DeckMatchConfig,
    deck_p0_id: &str,
    deck_p1_id: &str,
) -> Result<DeckMatchResult, String> {
    let first = run_rav_deck_matchup(config, deck_p0_id, deck_p1_id)?;
    let replay = run_rav_deck_matchup(config, deck_p0_id, deck_p1_id)?;
    if first.event_log != replay.event_log || first.digest != replay.digest {
        return Err(format!(
            "non-deterministic event log for {deck_p0_id} vs {deck_p1_id} at seed {}: first={} replay={}",
            config.shuffle_seed, first.digest, replay.digest
        ));
    }
    Ok(first)
}

/// The shared card index, built once per process.
///
/// Rebuilding it per policy per game is a measurable share of a multi-thousand
/// game campaign, and it is immutable, so one copy is shared by every seat.
pub fn shared_card_index() -> Arc<crate::CardIndex> {
    static INDEX: OnceLock<Arc<crate::CardIndex>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            let definitions = card_definitions();
            let mana: Vec<(&'static str, cardbench_magic_engine::ActivatedManaAbility)> =
                rav_mana_ability_bindings()
                    .into_iter()
                    .map(|binding| (binding.card_definition, binding.ability))
                    .collect();
            let additional: Vec<&'static str> = rav_additional_spell_cost_bindings()
                .into_iter()
                .map(|binding| binding.card_definition)
                .collect();
            let entry: Vec<(&'static str, u8)> = rav_land_entry_bindings()
                .into_iter()
                .filter_map(|binding| {
                    binding
                        .optional_life_payment
                        .map(|life| (binding.card_definition, life))
                })
                .collect();
            let tapped: Vec<&'static str> = rav_land_entry_bindings()
                .into_iter()
                .filter(|binding| binding.enters_tapped)
                .map(|binding| binding.card_definition)
                .collect();
            let activated: Vec<(&'static str, cardbench_magic_engine::ActivatedAbility)> =
                rav_activated_ability_bindings()
                    .into_iter()
                    .map(|binding| (binding.card_definition, binding.ability))
                    .collect();
            Arc::new(crate::CardIndex::build(
                &definitions,
                &mana,
                &additional,
                &entry,
                &tapped,
                &activated,
            ))
        })
        .clone()
}

/// Every deck fixture the runners can seat: coverage fixtures, catalog bands,
/// and constructed decks.
fn all_deck_fixtures() -> Result<Vec<DeckFixture>, String> {
    let mut decks = load_reference_decks().map_err(|error| error.to_string())?;
    decks.extend(load_catalog_coverage_decks().map_err(|error| error.to_string())?);
    decks.extend(load_constructed_decks().map_err(|error| error.to_string())?);
    Ok(decks)
}

/// Builds the pilot for one seated deck.
///
/// A deck that names an archetype is piloted by the shared archetype policy; a
/// legacy coverage fixture keeps its hand-written pilot. Archetype wins when
/// both are present, because the whole point of the archetype lane is that the
/// deck is measured independently of a bespoke pilot.
fn pilot_for(player: PlayerId, deck: &DeckFixture) -> Result<Box<dyn CodePolicy>, String> {
    if !deck.archetype.is_empty() {
        let archetype = Archetype::parse(&deck.archetype).ok_or_else(|| {
            format!(
                "deck `{}` names unknown archetype `{}`",
                deck.id, deck.archetype
            )
        })?;
        return Ok(crate::archetypes::seat_policy(
            crate::archetypes::PolicyVersion::latest(),
            player,
            archetype,
            shared_card_index(),
        ));
    }
    policy_for(player, &deck.policy)
}

/// Runs one matchup with each seat's policy version chosen explicitly.
///
/// The ladder needs to seat two *different generations* of the same archetype
/// policy on the same deck list, which the archetype-driven path cannot express
/// because it always picks the latest version.
///
/// # Errors
///
/// Returns an error when a deck id is unknown or setup fails.
pub fn run_versioned_matchup(
    config: DeckMatchConfig,
    deck_p0_id: &str,
    deck_p1_id: &str,
    versions: [crate::archetypes::PolicyVersion; 2],
    archetype: Archetype,
    index: Arc<crate::CardIndex>,
) -> Result<DeckMatchResult, String> {
    let pilots: [Box<dyn CodePolicy>; 2] = [
        crate::archetypes::seat_policy(versions[0], PlayerId(0), archetype, index.clone()),
        crate::archetypes::seat_policy(versions[1], PlayerId(1), archetype, index),
    ];
    run_deck_matchup_with(config, deck_p0_id, deck_p1_id, pilots)
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
        "rav.catalog-pressure.v1" => Ok(Box::new(RavCatalogPolicy::new(
            player,
            CatalogPolicyProfile::Pressure,
        ))),
        "rav.catalog-curve.v1" => Ok(Box::new(RavCatalogPolicy::new(
            player,
            CatalogPolicyProfile::Curve,
        ))),
        "rav.catalog-control.v1" => Ok(Box::new(RavCatalogPolicy::new(
            player,
            CatalogPolicyProfile::Control,
        ))),
        "rav.catalog-graveyard.v1" => Ok(Box::new(RavCatalogPolicy::new(
            player,
            CatalogPolicyProfile::Graveyard,
        ))),
        "rav.catalog-top-end.v1" => Ok(Box::new(RavCatalogPolicy::new(
            player,
            CatalogPolicyProfile::TopEnd,
        ))),
        "rav.catalog-patient.v1" => Ok(Box::new(RavCatalogPolicy::new(
            player,
            CatalogPolicyProfile::Patient,
        ))),
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
        let result = run_rav_deck_matchup_verified(
            DeckMatchConfig {
                shuffle_seed,
                ..DeckMatchConfig::default()
            },
            "rav_boros_helix",
            "rav_selesnya_convoke",
        )?;
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

/// Runs every exact-sixty catalog band through a deterministic full game.
///
/// The fixed Boros control opponent supplies stack interaction and a terminal
/// clock without adding creatures that could make an intentionally generic
/// blocker policy approximate complex evasion. Every submitted action still
/// goes through `Game::submit_policy_move`; replay verification rejects event
/// or digest drift before a result is admitted.
pub fn run_rav_catalog_gauntlet(
    seeds: impl IntoIterator<Item = u64>,
) -> Result<CatalogGauntletResult, String> {
    let seeds = seeds.into_iter().collect::<Vec<_>>();
    if seeds.is_empty() {
        return Err("catalog gauntlet requires at least one shuffle seed".to_owned());
    }
    let decks = load_catalog_coverage_decks().map_err(|error| error.to_string())?;
    let policy_profile_count = decks
        .iter()
        .map(|deck| deck.policy.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let mut jobs = Vec::new();
    for deck in &decks {
        for &shuffle_seed in &seeds {
            jobs.push((deck.id.clone(), shuffle_seed));
        }
    }
    let mut ordered_results = vec![None; jobs.len()];
    let worker_limit = reference_matrix_worker_limit(
        jobs.len(),
        std::thread::available_parallelism()
            .ok()
            .map(std::num::NonZeroUsize::get),
    );
    for batch_start in (0..jobs.len()).step_by(worker_limit) {
        let batch_end = (batch_start + worker_limit).min(jobs.len());
        std::thread::scope(|scope| {
            let handles = jobs[batch_start..batch_end]
                .iter()
                .cloned()
                .enumerate()
                .map(|(batch_index, (deck_id, shuffle_seed))| {
                    let index = batch_start + batch_index;
                    scope.spawn(move || {
                        let result = run_rav_deck_matchup_verified(
                            DeckMatchConfig {
                                shuffle_seed,
                                ..DeckMatchConfig::default()
                            },
                            &deck_id,
                            "rav_boros_char_control",
                        );
                        (index, result)
                    })
                })
                .collect::<Vec<_>>();
            for handle in handles {
                let (index, result) = handle.join().expect("catalog worker must not panic");
                ordered_results[index] = Some(result);
            }
        });
    }
    let mut matches = Vec::with_capacity(ordered_results.len());
    let mut findings = Vec::new();
    for result in ordered_results {
        let result = result
            .expect("every catalog job writes one ordered slot")
            .map_err(|error| format!("catalog gauntlet matchup failed: {error}"))?;
        findings.extend(result.engine_findings.iter().cloned());
        matches.push(result);
    }
    Ok(CatalogGauntletResult {
        deck_count: decks.len(),
        policy_profile_count,
        covered_definition_count: RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
        covered_printing_count: RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        tournament: fail_closed_tournament("rav_catalog_all_card_gauntlet", matches, findings),
    })
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
    let mut jobs = Vec::new();
    for deck_p0 in &deck_ids {
        for deck_p1 in &deck_ids {
            for &shuffle_seed in &seeds {
                jobs.push((deck_p0.clone(), deck_p1.clone(), shuffle_seed));
            }
        }
    }
    // Every matchup owns an independent Game, catalog, and policy pair. Keep
    // concurrency bounded: the public corpus can contain hundreds of jobs and
    // one OS thread per job makes the audit slower through scheduler and memory
    // pressure. Results are still written by job index for deterministic replay.
    let mut ordered_results = vec![None; jobs.len()];
    let available_parallelism = std::thread::available_parallelism()
        .ok()
        .map(std::num::NonZeroUsize::get);
    let worker_limit = reference_matrix_worker_limit(jobs.len(), available_parallelism);
    for batch_start in (0..jobs.len()).step_by(worker_limit) {
        let batch_end = (batch_start + worker_limit).min(jobs.len());
        std::thread::scope(|scope| {
            let handles = jobs[batch_start..batch_end]
                .iter()
                .cloned()
                .enumerate()
                .map(|(batch_index, (deck_p0, deck_p1, shuffle_seed))| {
                    let index = batch_start + batch_index;
                scope.spawn(move || {
                    let result = run_rav_deck_matchup_verified(
                        DeckMatchConfig {
                            shuffle_seed,
                            ..DeckMatchConfig::default()
                        },
                        &deck_p0,
                        &deck_p1,
                    )
                    .map_err(|error| {
                        format!(
                            "reference matrix {deck_p0} vs {deck_p1} at seed {shuffle_seed}: {error}"
                        )
                    });
                    (index, result)
                })
                })
                .collect::<Vec<_>>();
            for handle in handles {
                let (index, result) = handle
                    .join()
                    .expect("reference matrix worker must not panic");
                ordered_results[index] = Some(result);
            }
        });
    }
    let mut matches = Vec::with_capacity(ordered_results.len());
    let mut engine_findings = Vec::new();
    for result in ordered_results.into_iter().flatten() {
        let result = result?;
        engine_findings.extend(result.engine_findings.iter().cloned());
        matches.push(result);
    }
    Ok(fail_closed_tournament(
        RAV_REFERENCE_DECK_MATRIX_ID,
        matches,
        engine_findings,
    ))
}

const MAX_REFERENCE_MATRIX_WORKERS: usize = 16;

fn reference_matrix_worker_limit(job_count: usize, parallelism: Option<usize>) -> usize {
    let available = parallelism
        .unwrap_or(1)
        .clamp(1, MAX_REFERENCE_MATRIX_WORKERS);
    job_count.max(1).min(available)
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

/// Identity of one object that appeared in a match, for transcript review.
///
/// The event log names objects but never says what they are, so no
/// type-based statistic is derivable from a transcript alone. Resolving the
/// ids once at the end costs nothing and makes the whole log queryable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectIdentity {
    pub object: cardbench_magic_engine::ObjectId,
    pub definition: &'static str,
    pub owner: PlayerId,
}

/// Snapshots the identity of every object that currently exists.
///
/// Scans a bounded id range rather than the event log, and is called both at
/// setup and at the end. That matters: CR 800.4a removes a departing player's
/// objects, so an end-of-game scan alone silently loses every card owned by
/// the loser -- which is half the seats in every decisive match, and would
/// make any per-seat statistic quietly wrong rather than obviously missing.
fn object_identities(
    game: &Game,
    into: &mut BTreeMap<cardbench_magic_engine::ObjectId, ObjectIdentity>,
) {
    // Two 60-card decks plus tokens. Ids are dense and allocated from one
    // counter, so a bounded scan is complete without exposing the map.
    for raw in 0..OBJECT_SCAN_LIMIT {
        let object = cardbench_magic_engine::ObjectId(raw);
        let Ok(card) = game.object(object) else {
            continue;
        };
        let owner = card.owner;
        let Ok(definition) = game.card_definition(object) else {
            continue;
        };
        into.entry(object).or_insert(ObjectIdentity {
            object,
            definition: definition.id,
            owner,
        });
    }
}

/// Upper bound for the identity scan: two 60-card decks leave ample room for
/// tokens and virtual copies.
const OBJECT_SCAN_LIMIT: u64 = 512;

/// Pairs the ordinary result with the typed event log, so the capturing entry
/// point and the plain one cannot drift apart.
#[allow(clippy::too_many_arguments)] // One receipt assembly point; a struct here would only rename the same fields.
fn captured_with(
    identities: BTreeMap<cardbench_magic_engine::ObjectId, ObjectIdentity>,
    game: &Game,
    deck_ids: [String; 2],
    config: DeckMatchConfig,
    termination: DeckMatchTermination,
    attempted_policy_moves: u32,
    accepted_policy_moves: u32,
    engine_findings: Vec<EngineFinding>,
) -> (DeckMatchResult, Vec<GameEvent>, Vec<ObjectIdentity>) {
    let result = result_from_game(
        game,
        deck_ids,
        config,
        termination,
        attempted_policy_moves,
        accepted_policy_moves,
        engine_findings,
    );
    (
        result,
        game.event_log.clone(),
        identities.into_values().collect(),
    )
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
        assert_eq!(first.turns, 38);
        assert_eq!(first.accepted_policy_moves, 1025);
        assert_eq!(first.life, [0, 6]);
        assert_eq!(first.digest, "fnv1a64:7260cbfa01cc5c9e");
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
        assert!(sweep.passed());
    }

    #[test]
    fn matrix_cross_deck_match_must_not_reject_after_player_departure() {
        let result = run_rav_deck_matchup(
            DeckMatchConfig::default(),
            "rav_boros_helix",
            "rav_dimir_transmute_helix",
        )
        .expect("cross-deck matchup setup");
        println!("termination={:?}", result.termination);
        println!(
            "tail={:?}",
            result.event_log.iter().rev().take(24).collect::<Vec<_>>()
        );
        assert!(
            matches!(
                result.termination,
                DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
            ),
            "cross-deck match rejected: {:?}",
            result.termination
        );
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
    fn catalog_opponent_completes_private_discard_decisions() {
        let result = run_rav_deck_matchup(
            DeckMatchConfig::default(),
            "rav_catalog_band_17",
            "rav_boros_char_control",
        )
        .expect("catalog band 17 versus Char Control setup");
        assert!(
            result.is_clean_completion(),
            "catalog private-discard regression failed: termination={:?}, tail={:?}",
            result.termination,
            result.event_log.iter().rev().take(24).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "explicit 16-seed campaign; run with --ignored"]
    fn sixteen_seed_tournament_fails_closed_on_all_engine_or_policy_errors() {
        let tournament = run_rav_engine_tournament(0..16).expect("sixteen-seed engine probe");
        assert_eq!(tournament.matches.len(), 16);
        assert!(tournament.passed(), "{tournament:#?}");
    }

    #[test]
    #[ignore = "explicit 15x15 one-seed matrix campaign; run with --ignored"]
    fn every_reference_deck_pair_has_an_attributable_fail_closed_trace() {
        let deck_count = load_reference_decks()
            .expect("public reference decks")
            .len();
        let matrix = run_rav_reference_deck_matrix([0]).expect("one-seed reference matrix");
        assert_eq!(matrix.id, RAV_REFERENCE_DECK_MATRIX_ID);
        assert_eq!(matrix.matches.len(), deck_count * deck_count);
        assert!(matrix.passed(), "{matrix:#?}");
        assert!(matrix.matches.iter().all(|match_result| {
            matches!(
                match_result.termination,
                DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
            )
        }));
    }

    #[test]
    fn reference_matrix_caps_live_workers_instead_of_spawning_one_thread_per_match() {
        assert_eq!(reference_matrix_worker_limit(630, Some(8)), 8);
        assert_eq!(reference_matrix_worker_limit(7, Some(32)), 7);
        assert_eq!(reference_matrix_worker_limit(3, None), 1);
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
