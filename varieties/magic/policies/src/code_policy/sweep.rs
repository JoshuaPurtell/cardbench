//! Scores one (deck, pilot) candidate against a roster's opponent split.
//!
//! # What a sweep computes
//!
//! Two arms over the same opponent surface:
//!
//! ```text
//! candidate arm   (candidate deck, candidate pilot)  vs every opponent cell
//! reference arm   (reference deck, reference_v1)     vs every opponent cell
//! reward          mean over opponents of (candidate rate - reference rate)
//! ```
//!
//! A **delta, never an absolute.** An absolute win rate is not comparable across
//! runs: it moves when the opponent split changes, when the seed count changes,
//! and when the engine changes. Anchoring on a frozen reference that played the
//! identical cells removes all three.
//!
//! # What is reused rather than rebuilt
//!
//! Everything measurement-shaped already exists in `ladder.rs` and `matchup.rs`
//! and is reused here rather than reimplemented:
//!
//! * **Paired seats.** Every (opponent, deck, seed) is played from both seats,
//!   so the play advantage cancels.
//! * **Wilson intervals** on every reported rate ([`Interval::wilson`]).
//! * **Per-opponent reporting.** `ladder.rs` refuses to let "helps Boros, hurts
//!   Golgari" hide inside a mean; the same rule applies per opponent here, and
//!   [`SweepReport::regressions`] is the enforcement.
//! * **Contamination is not a score.** An engine-refused proposal means the game
//!   ended by stalling rather than by play, so those cells are counted and
//!   surfaced instead of being folded into an average.
//!
//! # Fail-closed coverage
//!
//! [`Coverage`] is checked before anything is scored. If an arm does not
//! reproduce the roster's cell set *exactly* — no missing cell, no extra cell,
//! and every candidate cell paired to a reference cell — the sweep reports
//! **zero**, not a partial result. A partial sweep that reported the mean of the
//! cells it managed to run would be maximally misleading: the cells that failed
//! are not missing at random, they are the hard ones.
//!
//! # Gate
//!
//! A paired bootstrap over per-cell score deltas, joined on
//! [`Cell::pair_key`]. The candidate passes only when the lower bound of the
//! 95% interval is strictly above zero *and* no opponent regressed
//! significantly. Deterministic given its fixed resampling seed, so two runs of
//! the same sweep agree.

use crate::archetype::Archetype;
use crate::archetypes::seat_policy;
use crate::planner::CardIndex;
use std::sync::Arc;
use crate::code_policy::roster::{Cell, Entrant, Seat, Surface};
use crate::deck_match::{
    DeckMatchConfig, DeckMatchTermination, run_deck_matchup_with, shared_card_index,
};
use crate::matchup::{Edge, Interval};
use crate::{CodePolicy, EngineFinding};
use cardbench_magic_engine::PlayerId;
use std::collections::BTreeMap;

/// A count-over-count ratio, without a lossy `usize as f64`.
///
/// Cell counts are in the hundreds, so the conversion is exact; going through
/// `u32` makes that a checked fact rather than an assumption.
fn ratio(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    let part = u32::try_from(part).map_or(f64::from(u32::MAX), f64::from);
    let whole = u32::try_from(whole).map_or(f64::from(u32::MAX), f64::from);
    part / whole
}

/// Ceiling on unresolved games before a sweep refuses to report a number.
///
/// A stalled game scores as a non-win, so a high stall rate quietly flattens
/// every cell toward zero and makes two arms look alike. Past this fraction the
/// measurement is not about play any more.
pub const MAX_STALL_FRACTION: f64 = 0.25;

/// Resampling draws for the paired bootstrap.
pub const BOOTSTRAP_DRAWS: u32 = 2_000;

/// Fixed resampling seed. The gate must be reproducible: a verdict that changes
/// between two runs of the same data is not a verdict.
const BOOTSTRAP_SEED: u64 = 0x5EED_C0DE_1234_5678;

/// What one cell produced.
#[derive(Clone, Debug)]
pub struct CellOutcome {
    pub cell: Cell,
    /// Win 1.0, draw 0.5, loss or unresolved 0.0. The reward currency.
    pub score: f64,
    /// Whether the game reached a winner. Draws and stalls are not decided.
    pub decided: bool,
    pub drawn: bool,
    /// The game did not reach a rules-valid terminal state within the bounds.
    pub stalled: bool,
    pub turns: u32,
    /// Proposals the engine refused. Any nonzero value contaminates the cell.
    pub rejected_moves: u32,
    pub engine_findings: Vec<EngineFinding>,
    /// Why a cell failed to reach a rules-valid terminal state.
    ///
    /// A bare count of contaminated cells says a measurement is untrustworthy
    /// without saying why, which is one step short of useful: the reason is the
    /// only part anyone can act on.
    pub termination_detail: Option<String>,
}

impl CellOutcome {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.rejected_moves == 0 && self.engine_findings.is_empty() && !self.stalled
    }
}

/// One arm's complete result over a surface.
#[derive(Clone, Debug)]
pub struct Arm {
    pub entrant: Entrant,
    pub outcomes: Vec<CellOutcome>,
}

impl Arm {
    fn by_pair_key(&self) -> BTreeMap<String, &CellOutcome> {
        self.outcomes
            .iter()
            .map(|outcome| (outcome.cell.pair_key(), outcome))
            .collect()
    }

    fn stall_fraction(&self) -> f64 {
        if self.outcomes.is_empty() {
            return 1.0;
        }
        let stalled = self.outcomes.iter().filter(|cell| cell.stalled).count();
        ratio(stalled, self.outcomes.len())
    }
}

/// Whether the sweep reproduced the roster's cell set exactly.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    pub expected: usize,
    pub candidate_produced: usize,
    pub reference_produced: usize,
    pub missing: Vec<String>,
    pub unexpected: Vec<String>,
    /// Candidate cells with no reference cell at the same shared coordinate.
    pub unpaired: Vec<String>,
    /// A stall rate past [`MAX_STALL_FRACTION`] on either arm.
    pub stall_failures: Vec<String>,
}

impl Coverage {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.expected > 0
            && self.candidate_produced == self.expected
            && self.reference_produced == self.expected
            && self.missing.is_empty()
            && self.unexpected.is_empty()
            && self.unpaired.is_empty()
            && self.stall_failures.is_empty()
    }
}

/// One opponent's slice of the comparison.
#[derive(Clone, Debug)]
pub struct OpponentReport {
    pub opponent_id: String,
    pub opponent_deck: String,
    pub opponent_pilot: String,
    /// Candidate win rate over decided games, Wilson.
    pub candidate: Interval,
    pub reference: Interval,
    /// Candidate rate minus reference rate, with a 95% interval.
    pub delta: Edge,
    /// The pairing's blind spot, reported for the same reason `ladder.rs`
    /// reports it: a seat-dependent change cancels to zero and looks like a
    /// no-op.
    pub candidate_on_play: Interval,
    pub candidate_on_draw: Interval,
    pub cells: usize,
    pub candidate_draws: u32,
    pub candidate_stalls: u32,
    pub candidate_rejected_moves: u32,
    pub engine_findings: Vec<EngineFinding>,
    /// Distinct stall reasons seen against this opponent, with counts.
    pub stall_reasons: BTreeMap<String, u32>,
}

impl OpponentReport {
    /// A significant loss against this opponent. One is enough to block the
    /// gate — that is the rule this type exists to enforce.
    #[must_use]
    pub fn is_regression(&self) -> bool {
        self.delta.is_established() && self.delta.point < 0.0
    }

    #[must_use]
    pub fn is_contaminated(&self) -> bool {
        self.candidate_rejected_moves > 0 || !self.engine_findings.is_empty()
    }
}

/// The whole verdict.
#[derive(Clone, Debug)]
pub struct SweepReport {
    pub split: String,
    pub roster_id: String,
    pub task_id: String,
    pub score_metric: String,
    pub manifest_sha256: Option<String>,
    pub candidate: Entrant,
    pub reference: Entrant,
    pub coverage: Coverage,
    pub per_opponent: Vec<OpponentReport>,
    /// Pooled candidate and reference win rates, for context only. The reward is
    /// the delta.
    pub candidate_overall: Interval,
    pub reference_overall: Interval,
    /// Mean over opponents of (candidate rate - reference rate). Zero when
    /// coverage failed.
    pub reward: f64,
    /// Paired bootstrap over per-cell score deltas.
    pub delta_low: f64,
    pub delta_high: f64,
    /// Whether a number was produced at all. False means coverage failed and
    /// `reward` is the fail-closed zero, not a measurement.
    pub scored: bool,
}

impl SweepReport {
    /// Opponents the candidate is significantly worse against.
    #[must_use]
    pub fn regressions(&self) -> Vec<&OpponentReport> {
        self.per_opponent
            .iter()
            .filter(|report| report.is_regression())
            .collect()
    }

    #[must_use]
    pub fn contaminated(&self) -> Vec<&OpponentReport> {
        self.per_opponent
            .iter()
            .filter(|report| report.is_contaminated())
            .collect()
    }

    /// The acceptance gate: a scored sweep, a bootstrap lower bound strictly
    /// above zero, and no per-opponent regression.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.scored && self.delta_low > 0.0 && self.regressions().is_empty()
    }
}

/// Builds one seat for an entrant that is not a compiled-in generation.
///
/// The submission seam. Without it a candidate can only ever be a
/// [`PolicyVersion`](crate::archetypes::PolicyVersion) variant that is already
/// in this crate, which is fine for comparing generations to each other and
/// useless for grading a policy someone else wrote.
pub type SeatFactory<'a> =
    &'a (dyn Fn(PlayerId, Archetype, Arc<CardIndex>) -> Box<dyn CodePolicy> + Sync);

/// Plays every cell for one arm.
///
/// # Errors
///
/// Propagates any setup failure from the match runner.
pub fn run_arm(
    surface: &Surface,
    entrant: &Entrant,
    config: &DeckMatchConfig,
) -> Result<Arm, String> {
    run_arm_with(surface, entrant, config, None)
}

/// [`run_arm`], with the entrant's seat optionally built by `seat` instead of
/// resolved from `entrant.pilot`.
///
/// Only the entrant's seat is affected. Opponents are always the roster's
/// pilots, and the reference arm is always the frozen generation — an origin a
/// caller could substitute is not an origin.
///
/// # Errors
///
/// Propagates any setup failure from the match runner.
pub fn run_arm_with(
    surface: &Surface,
    entrant: &Entrant,
    config: &DeckMatchConfig,
    seat: Option<SeatFactory>,
) -> Result<Arm, String> {
    let index = shared_card_index();
    let mut outcomes = Vec::new();
    for cell in surface.cells(entrant) {
        let opponent = surface
            .opponents
            .iter()
            .find(|candidate| candidate.id == cell.opponent_id)
            .ok_or_else(|| format!("cell names unknown opponent `{}`", cell.opponent_id))?;

        let entrant_seat = cell.seat.index();
        let opponent_seat = 1 - entrant_seat;
        let mut decks = [String::new(), String::new()];
        decks[entrant_seat].clone_from(&entrant.deck);
        decks[opponent_seat].clone_from(&opponent.deck);

        // `entrant.pilot` is deliberately not consulted when a factory is
        // supplied: a submitted candidate has no generation, and silently
        // falling back to one would grade a compiled-in policy while reporting
        // the candidate's label.
        let entrant_pilot: Box<dyn CodePolicy> = match seat {
            Some(build) => build(PlayerId(entrant_seat), entrant.archetype, index.clone()),
            None => seat_policy(
                entrant.pilot,
                PlayerId(entrant_seat),
                entrant.archetype,
                index.clone(),
            ),
        };
        let opponent_pilot: Box<dyn CodePolicy> = seat_policy(
            opponent.pilot,
            PlayerId(opponent_seat),
            opponent.archetype,
            index.clone(),
        );
        let pilots: [Box<dyn CodePolicy>; 2] = if entrant_seat == 0 {
            [entrant_pilot, opponent_pilot]
        } else {
            [opponent_pilot, entrant_pilot]
        };

        let result = run_deck_matchup_with(
            DeckMatchConfig {
                shuffle_seed: cell.seed,
                ..*config
            },
            &decks[0],
            &decks[1],
            pilots,
        )?;

        let rejected = result
            .attempted_policy_moves
            .saturating_sub(result.accepted_policy_moves);
        let (score, decided, drawn, stalled) = match result.termination {
            DeckMatchTermination::Winner(PlayerId(seat)) => {
                (f64::from(u8::from(seat == entrant_seat)), true, false, false)
            }
            DeckMatchTermination::Draw => (0.5, false, true, false),
            _ => (0.0, false, false, true),
        };
        let termination_detail = if stalled {
            Some(format!("{:?}", result.termination))
        } else {
            None
        };
        outcomes.push(CellOutcome {
            cell,
            score,
            decided,
            drawn,
            stalled,
            turns: result.turns,
            rejected_moves: rejected,
            engine_findings: result.engine_findings,
            termination_detail,
        });
    }
    Ok(Arm {
        entrant: entrant.clone(),
        outcomes,
    })
}

/// Runs both arms and scores the candidate against the frozen reference.
///
/// # Errors
///
/// Propagates any setup failure from the match runner. A coverage failure is
/// **not** an error: it is a scored-zero report, because the caller still needs
/// the diagnostics that say which cells went missing.
pub fn run_sweep(
    surface: &Surface,
    candidate: &Entrant,
    config: &DeckMatchConfig,
) -> Result<SweepReport, String> {
    run_sweep_with(surface, candidate, config, None)
}

/// [`run_sweep`], with the candidate arm's seat optionally built by `seat`.
///
/// The reference arm never takes the factory. Both arms still play the same
/// cells in the same order with the same seeds, which is what makes the
/// per-cell pairing in [`score`] meaningful.
///
/// # Errors
///
/// Propagates any setup failure from the match runner. A coverage failure is
/// **not** an error.
pub fn run_sweep_with(
    surface: &Surface,
    candidate: &Entrant,
    config: &DeckMatchConfig,
    seat: Option<SeatFactory>,
) -> Result<SweepReport, String> {
    let candidate_arm = run_arm_with(surface, candidate, config, seat)?;
    let reference_arm = run_arm(surface, &surface.reference, config)?;
    Ok(score(surface, &candidate_arm, &reference_arm))
}

/// Scores two completed arms. Separated from [`run_sweep`] so the scoring rules
/// are testable without playing six hundred games.
#[must_use]
#[allow(clippy::too_many_lines)] // One pass over the opponents is the readable audit trail.
pub fn score(surface: &Surface, candidate_arm: &Arm, reference_arm: &Arm) -> SweepReport {
    let coverage = coverage(surface, candidate_arm, reference_arm);
    let reference_by_key = reference_arm.by_pair_key();

    let mut per_opponent = Vec::new();
    let mut candidate_wins = 0;
    let mut candidate_decided = 0;
    let mut reference_wins = 0;
    let mut reference_decided = 0;

    for opponent in &surface.opponents {
        let cells: Vec<&CellOutcome> = candidate_arm
            .outcomes
            .iter()
            .filter(|outcome| outcome.cell.opponent_id == opponent.id)
            .collect();
        let mut wins = 0;
        let mut decided = 0;
        let mut seat_wins = [0_u32; 2];
        let mut seat_decided = [0_u32; 2];
        let mut draws = 0;
        let mut stalls = 0;
        let mut rejected = 0;
        let mut findings = Vec::new();
        let mut stall_reasons: BTreeMap<String, u32> = BTreeMap::new();
        let mut opponent_reference_wins = 0;
        let mut opponent_reference_decided = 0;

        for outcome in &cells {
            let seat = outcome.cell.seat.index();
            if outcome.decided {
                decided += 1;
                seat_decided[seat] += 1;
                let won = u32::from(outcome.score > 0.99);
                wins += won;
                seat_wins[seat] += won;
            }
            draws += u32::from(outcome.drawn);
            stalls += u32::from(outcome.stalled);
            rejected += outcome.rejected_moves;
            findings.extend(outcome.engine_findings.iter().cloned());
            if let Some(detail) = &outcome.termination_detail {
                *stall_reasons.entry(detail.clone()).or_default() += 1;
            }

            if let Some(mirror) = reference_by_key.get(&outcome.cell.pair_key()) {
                if mirror.decided {
                    opponent_reference_decided += 1;
                    opponent_reference_wins += u32::from(mirror.score > 0.99);
                }
            }
        }

        candidate_wins += wins;
        candidate_decided += decided;
        reference_wins += opponent_reference_wins;
        reference_decided += opponent_reference_decided;

        let candidate_rate = Interval::wilson(wins, decided);
        let reference_rate = Interval::wilson(opponent_reference_wins, opponent_reference_decided);
        per_opponent.push(OpponentReport {
            opponent_id: opponent.id.clone(),
            opponent_deck: opponent.deck.clone(),
            opponent_pilot: opponent.pilot.id().to_owned(),
            candidate: candidate_rate,
            reference: reference_rate,
            delta: difference(candidate_rate, reference_rate),
            candidate_on_play: Interval::wilson(
                seat_wins[Seat::Play.index()],
                seat_decided[Seat::Play.index()],
            ),
            candidate_on_draw: Interval::wilson(
                seat_wins[Seat::Draw.index()],
                seat_decided[Seat::Draw.index()],
            ),
            cells: cells.len(),
            candidate_draws: draws,
            candidate_stalls: stalls,
            candidate_rejected_moves: rejected,
            engine_findings: findings,
            stall_reasons,
        });
    }

    let complete = coverage.is_complete();
    let deltas: Vec<f64> = if complete {
        candidate_arm
            .outcomes
            .iter()
            .filter_map(|outcome| {
                reference_by_key
                    .get(&outcome.cell.pair_key())
                    .map(|mirror| outcome.score - mirror.score)
            })
            .collect()
    } else {
        Vec::new()
    };
    let (delta_low, delta_high) = if complete && !deltas.is_empty() {
        bootstrap_interval(&deltas)
    } else {
        (0.0, 0.0)
    };

    // Fail-closed. The reward is the mean *over opponents*, not over cells, so
    // one opponent with more resolved games cannot dominate the number.
    //
    // Note the two denominators, which are deliberate rather than sloppy: the
    // reported win rates and this reward are over *decided* games, because a
    // stalled game says nothing about who plays better; the bootstrap below is
    // over *every* cell's score, because the gate has to notice a candidate that
    // buys its edge by stalling more often than the reference.
    let reward = if complete && !per_opponent.is_empty() {
        let total: f64 = per_opponent.iter().map(|report| report.delta.point).sum();
        total * ratio(1, per_opponent.len())
    } else {
        0.0
    };

    SweepReport {
        split: surface.split.id().to_owned(),
        roster_id: surface.roster_id.clone(),
        task_id: surface.task_id.clone(),
        score_metric: surface.score_metric.clone(),
        manifest_sha256: surface.manifest_sha256.clone(),
        candidate: candidate_arm.entrant.clone(),
        reference: reference_arm.entrant.clone(),
        coverage,
        per_opponent,
        candidate_overall: Interval::wilson(candidate_wins, candidate_decided),
        reference_overall: Interval::wilson(reference_wins, reference_decided),
        reward,
        delta_low,
        delta_high,
        scored: complete,
    }
}

fn coverage(surface: &Surface, candidate_arm: &Arm, reference_arm: &Arm) -> Coverage {
    let expected_candidate: Vec<String> = surface
        .cells(&candidate_arm.entrant)
        .iter()
        .map(Cell::cell_id)
        .collect();
    let expected_reference: std::collections::BTreeSet<String> = surface
        .cells(&reference_arm.entrant)
        .iter()
        .map(Cell::cell_id)
        .collect();

    let produced_candidate: std::collections::BTreeSet<String> = candidate_arm
        .outcomes
        .iter()
        .map(|outcome| outcome.cell.cell_id())
        .collect();
    let produced_reference: std::collections::BTreeSet<String> = reference_arm
        .outcomes
        .iter()
        .map(|outcome| outcome.cell.cell_id())
        .collect();

    let expected_set: std::collections::BTreeSet<String> =
        expected_candidate.iter().cloned().collect();
    let mut missing: Vec<String> = expected_set.difference(&produced_candidate).cloned().collect();
    missing.extend(expected_reference.difference(&produced_reference).cloned());
    let mut unexpected: Vec<String> =
        produced_candidate.difference(&expected_set).cloned().collect();
    unexpected.extend(produced_reference.difference(&expected_reference).cloned());

    let reference_keys: std::collections::BTreeSet<String> = reference_arm
        .outcomes
        .iter()
        .map(|outcome| outcome.cell.pair_key())
        .collect();
    let unpaired: Vec<String> = candidate_arm
        .outcomes
        .iter()
        .map(|outcome| outcome.cell.pair_key())
        .filter(|key| !reference_keys.contains(key))
        .collect();

    let mut stall_failures = Vec::new();
    for (label, arm) in [("candidate", candidate_arm), ("reference", reference_arm)] {
        let fraction = arm.stall_fraction();
        if !arm.outcomes.is_empty() && fraction > MAX_STALL_FRACTION {
            stall_failures.push(format!(
                "{label} arm stalled on {:.1}% of cells (ceiling {:.0}%)",
                fraction * 100.0,
                MAX_STALL_FRACTION * 100.0
            ));
        }
    }

    Coverage {
        expected: expected_candidate.len(),
        candidate_produced: produced_candidate.len(),
        reference_produced: produced_reference.len(),
        missing,
        unexpected,
        unpaired,
        stall_failures,
    }
}

/// The difference between two independent proportions, with a 95% interval.
///
/// Deliberately the same normal-approximation form `ladder.rs` uses for its seat
/// asymmetry: the two halves are separate games rather than complementary seats
/// of one game, so the variances add.
fn difference(left: Interval, right: Interval) -> Edge {
    if left.samples == 0 || right.samples == 0 {
        return Edge {
            point: 0.0,
            low: -2.0,
            high: 2.0,
            samples: 0,
        };
    }
    let z = 1.959_963_984_540_054_f64;
    let variance =
        |interval: &Interval| interval.point * (1.0 - interval.point) / f64::from(interval.samples);
    let spread = z * (variance(&left) + variance(&right)).sqrt();
    let point = left.point - right.point;
    Edge {
        point,
        low: (point - spread).max(-1.0),
        high: (point + spread).min(1.0),
        samples: left.samples + right.samples,
    }
}

/// Deterministic xorshift64*, so the gate does not depend on a global RNG.
struct Resampler(u64);

impl Resampler {
    fn next_index(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let value = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        // Modulo bias is negligible against a 64-bit draw and a bound in the
        // hundreds, and this is a resampling index, not a key.
        (value >> 11) as usize % bound
    }
}

/// 95% paired-bootstrap interval on the mean of `deltas`.
fn bootstrap_interval(deltas: &[f64]) -> (f64, f64) {
    let mut generator = Resampler(BOOTSTRAP_SEED);
    let mut means = Vec::with_capacity(BOOTSTRAP_DRAWS as usize);
    let scale = ratio(1, deltas.len());
    for _ in 0..BOOTSTRAP_DRAWS {
        let mut total = 0.0;
        for _ in 0..deltas.len() {
            total += deltas[generator.next_index(deltas.len())];
        }
        means.push(total * scale);
    }
    means.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let low = means[(BOOTSTRAP_DRAWS as usize * 25) / 1_000];
    let high = means[((BOOTSTRAP_DRAWS as usize * 975) / 1_000).min(means.len() - 1)];
    (low, high)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_policy::roster::{Opponent, Split};
    use crate::{Archetype, PolicyVersion};

    fn entrant(label: &str, deck: &str, pilot: PolicyVersion) -> Entrant {
        Entrant {
            label: label.to_owned(),
            deck: deck.to_owned(),
            pilot,
            archetype: Archetype::Aggro,
        }
    }

    fn surface(seeds: u32) -> Surface {
        Surface {
            split: Split::Train,
            roster_id: "test".to_owned(),
            task_id: "cardbench/magic/code_policy".to_owned(),
            score_metric: "cell_win_rate_delta".to_owned(),
            manifest_sha256: None,
            reference: entrant("reference_v1", "deck_ref", PolicyVersion::V5),
            opponents: vec![
                Opponent {
                    id: "opp_a".to_owned(),
                    deck: "deck_x".to_owned(),
                    pilot: PolicyVersion::V1,
                    archetype: Archetype::Aggro,
                },
                Opponent {
                    id: "opp_b".to_owned(),
                    deck: "deck_y".to_owned(),
                    pilot: PolicyVersion::V2,
                    archetype: Archetype::Midrange,
                },
            ],
            seeds,
            candidate_deck_pool: vec!["deck_c".to_owned()],
        }
    }

    /// Builds an arm whose outcome for each cell is decided by `won`.
    fn arm(surface: &Surface, entrant: &Entrant, won: impl Fn(&Cell) -> bool) -> Arm {
        Arm {
            entrant: entrant.clone(),
            outcomes: surface
                .cells(entrant)
                .into_iter()
                .map(|cell| {
                    let win = won(&cell);
                    CellOutcome {
                        cell,
                        score: if win { 1.0 } else { 0.0 },
                        decided: true,
                        drawn: false,
                        stalled: false,
                        turns: 12,
                        rejected_moves: 0,
                        engine_findings: Vec::new(),
                        termination_detail: None,
                    }
                })
                .collect(),
        }
    }

    /// The headline requirement: a candidate that beats the reference on every
    /// cell scores a positive delta and clears the gate.
    #[test]
    fn a_uniformly_better_candidate_scores_a_positive_delta() {
        let surface = surface(30);
        let candidate = entrant("candidate", "deck_c", PolicyVersion::V8);
        let report = score(
            &surface,
            &arm(&surface, &candidate, |_| true),
            &arm(&surface, &surface.reference, |_| false),
        );
        assert!(report.scored);
        assert!(report.coverage.is_complete());
        assert!((report.reward - 1.0).abs() < 1e-9);
        assert!(report.delta_low > 0.0);
        assert!(report.passes());
    }

    /// The reward must be a delta. A candidate that merely matches the
    /// reference scores zero however high its absolute win rate is.
    #[test]
    fn matching_the_reference_scores_zero_however_good_the_absolute_rate() {
        let surface = surface(30);
        let candidate = entrant("candidate", "deck_c", PolicyVersion::V8);
        let report = score(
            &surface,
            &arm(&surface, &candidate, |_| true),
            &arm(&surface, &surface.reference, |_| true),
        );
        assert!((report.candidate_overall.point - 1.0).abs() < 1e-9);
        assert!(report.reward.abs() < 1e-9);
        assert!(!report.passes(), "a tie is not a lift");
    }

    /// `ladder.rs`'s rule, carried over: an aggregate gain that hides a
    /// significant per-opponent loss is not progress.
    #[test]
    fn a_per_opponent_regression_blocks_the_gate_even_with_a_positive_mean() {
        let surface = surface(30);
        let candidate = entrant("candidate", "deck_c", PolicyVersion::V8);
        let report = score(
            &surface,
            &arm(&surface, &candidate, |cell| cell.opponent_id == "opp_a"),
            &arm(&surface, &surface.reference, |cell| {
                cell.opponent_id == "opp_b"
            }),
        );
        assert!(report.scored);
        assert!(report.reward.abs() < 1e-9, "the mean cancels exactly");
        assert_eq!(report.regressions().len(), 1);
        assert_eq!(report.regressions()[0].opponent_id, "opp_b");
        assert!(!report.passes());
    }

    /// Fail-closed coverage. A sweep missing one cell scores zero, not the mean
    /// of the cells it managed.
    #[test]
    fn one_missing_cell_scores_zero_rather_than_a_partial_result() {
        let surface = surface(30);
        let candidate = entrant("candidate", "deck_c", PolicyVersion::V8);
        let mut candidate_arm = arm(&surface, &candidate, |_| true);
        let dropped = candidate_arm.outcomes.pop().expect("arm has cells");
        let report = score(
            &surface,
            &candidate_arm,
            &arm(&surface, &surface.reference, |_| false),
        );
        assert!(!report.scored);
        assert!(!report.coverage.is_complete());
        assert_eq!(report.coverage.missing, vec![dropped.cell.cell_id()]);
        assert!(
            report.reward.abs() < 1e-9,
            "a partial sweep must score zero, not 100% of what it ran"
        );
        assert!(!report.passes());
    }

    /// An extra cell is just as disqualifying as a missing one: it means the
    /// sweep ran something the roster does not define.
    #[test]
    fn an_unexpected_cell_scores_zero() {
        let surface = surface(2);
        let candidate = entrant("candidate", "deck_c", PolicyVersion::V8);
        let mut candidate_arm = arm(&surface, &candidate, |_| true);
        let mut extra = candidate_arm.outcomes[0].clone();
        extra.cell.seed = 999;
        candidate_arm.outcomes.push(extra);
        let report = score(
            &surface,
            &candidate_arm,
            &arm(&surface, &surface.reference, |_| false),
        );
        assert!(!report.scored);
        assert_eq!(report.coverage.unexpected.len(), 1);
        assert!(report.reward.abs() < 1e-9);
    }

    /// A stall rate past the ceiling is not a score either: stalls read as
    /// non-wins and would flatten both arms toward each other.
    #[test]
    fn a_stalled_arm_refuses_to_score() {
        let surface = surface(4);
        let candidate = entrant("candidate", "deck_c", PolicyVersion::V8);
        let mut candidate_arm = arm(&surface, &candidate, |_| true);
        for outcome in candidate_arm.outcomes.iter_mut().take(8) {
            outcome.stalled = true;
            outcome.decided = false;
            outcome.score = 0.0;
        }
        let report = score(
            &surface,
            &candidate_arm,
            &arm(&surface, &surface.reference, |_| false),
        );
        assert!(!report.scored);
        assert_eq!(report.coverage.stall_failures.len(), 1);
        assert!(report.reward.abs() < 1e-9);
    }

    /// The gate must not fire on noise: a small mean over cells that disagree
    /// in sign has an interval that straddles zero.
    ///
    /// Note what the *other* direction implies, because it is not a bug. When
    /// the candidate never loses a cell to the reference the delta vector is
    /// one-sided, and a one-sided vector has a strictly positive bootstrap
    /// lower bound however few cells there are. That is the resampling working
    /// as specified — "won some, lost none, on identical shuffles against
    /// identical opponents" is genuinely not consistent with no effect — and it
    /// is why the gate carries the per-opponent regression condition as well:
    /// the bootstrap alone is a statement about the mean.
    #[test]
    fn a_noisy_edge_does_not_clear_the_gate() {
        let noisy = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, 0.0];
        let mean: f64 = noisy.iter().sum::<f64>() * ratio(1, noisy.len());
        assert!(mean > 0.0, "the point estimate favours the candidate");
        let (low, high) = bootstrap_interval(&noisy);
        assert!(low < 0.0 && high > 0.0, "but the interval straddles zero");
    }

    /// A one-sided win over the whole surface with a per-opponent loss still
    /// fails, which is the condition the bootstrap cannot express on its own.
    #[test]
    fn the_regression_condition_is_independent_of_the_bootstrap() {
        let surface = surface(30);
        let candidate = entrant("candidate", "deck_c", PolicyVersion::V8);
        let report = score(
            &surface,
            &arm(&surface, &candidate, |cell| cell.opponent_id == "opp_a"),
            &arm(&surface, &surface.reference, |cell| {
                cell.opponent_id == "opp_b" && cell.seed < 20
            }),
        );
        assert!(report.scored);
        assert!(report.reward > 0.0, "the mean favours the candidate");
        assert!(report.delta_low > 0.0, "and the bootstrap agrees");
        assert_eq!(report.regressions().len(), 1, "but opp_b got worse");
        assert!(!report.passes());
    }

    /// The bootstrap must be reproducible or the verdict is not a verdict.
    #[test]
    fn the_bootstrap_is_deterministic() {
        let deltas: Vec<f64> = (0..200).map(|index| f64::from(index % 3) - 1.0).collect();
        assert_eq!(bootstrap_interval(&deltas), bootstrap_interval(&deltas));
    }
}
