//! Policy-version ladder: does generation N actually beat generation N-1?
//!
//! This is the fitness function for improving policies. The measurement holds
//! the *deck* fixed and varies only the pilot, so a win rate is attributable to
//! the policy change and nothing else:
//!
//! * **Mirror decks.** Both seats play the same deck list. Any asymmetry is the
//!   pilot, because there is no other asymmetry left.
//! * **Every deck, not one.** A change that helps Boros aggro and quietly hurts
//!   Golgari midrange is not an improvement. Per-deck rates are reported, not
//!   just the aggregate, so a regression cannot hide inside an average.
//! * **Paired seats.** Each seed is played twice with the versions swapped, so
//!   the play advantage and the shuffle both cancel.
//! * **Wilson intervals.** A ladder step is only a real gain when its interval
//!   excludes 50%.
//!
//! # What the paired rate cannot see
//!
//! Swapping the versions across seats is what makes the paired rate
//! attributable, and it is also a blind spot. An improvement that depends on
//! *which seat you sit in* -- attacking first into an untapped board, holding
//! interaction against an opponent who has already committed -- appears once as
//! a challenger win and once as a challenger loss on the same seed, and cancels
//! exactly. Such a change measures 50.0% and is indistinguishable from a no-op.
//!
//! So every verdict also reports the challenger's rate split by seat. The games
//! are already being played; discarding the split threw away the only evidence
//! that separates "this change did nothing" from "this change did something the
//! pairing hides". A rung that reads 50% paired but has a separated seat split
//! changed real behaviour and needs a different measurement, not a shrug.

use crate::archetype::Archetype;
use crate::archetypes::PolicyVersion;
use crate::deck_match::{run_versioned_matchup, shared_card_index};
use crate::matchup::{Edge, Interval};
use crate::{DeckMatchConfig, DeckMatchTermination, EngineFinding};
use cardbench_magic_engine::PlayerId;

/// One deck's verdict on a version pair.
#[derive(Clone, Debug)]
pub struct DeckVerdict {
    pub deck: String,
    pub archetype: Archetype,
    /// The challenger's win rate piloting this deck against the incumbent
    /// piloting the same deck.
    pub win_rate: Interval,
    /// The challenger's win rate in the games where it sat on the play, and on
    /// the draw. `win_rate` is the pooled pair; these two are what the pooling
    /// cancels.
    pub win_rate_on_the_play: Interval,
    pub win_rate_on_the_draw: Interval,
    pub games: u32,
    pub mean_turns: f64,
    /// Engine-refused proposals. Any nonzero value invalidates the cell: those
    /// games ended by stalling, not by play.
    pub rejected_moves: u32,
    pub truncated: u32,
    pub engine_findings: Vec<EngineFinding>,
}

impl DeckVerdict {
    /// How much better the challenger does on the play than on the draw, in
    /// win-rate points, with a 95% interval.
    ///
    /// Zero is the null the pairing assumes. A separated interval means the
    /// change is seat-dependent, so the pooled `win_rate` is averaging two
    /// different effects and understates both.
    #[must_use]
    pub fn seat_asymmetry(&self) -> Edge {
        difference(self.win_rate_on_the_play, self.win_rate_on_the_draw)
    }

    /// Whether this cell changed seat-dependent behaviour that the pooled rate
    /// cannot report.
    #[must_use]
    pub fn is_seat_asymmetric(&self) -> bool {
        self.seat_asymmetry().is_established()
    }
}

/// The difference between two independent proportions, with a 95% interval.
///
/// Unlike [`Matchup::play_edge`], the two halves here are separate games rather
/// than complementary seats of the same game, so the difference does not reduce
/// to one proportion and the variances add. The normal approximation is
/// adequate at ladder sample sizes and is used with its own bounds clamped to
/// the reachable range.
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

/// One rung: challenger against incumbent across every deck.
#[derive(Clone, Debug)]
pub struct LadderStep {
    pub challenger: PolicyVersion,
    pub incumbent: PolicyVersion,
    pub per_deck: Vec<DeckVerdict>,
    /// Pooled win rate across every deck.
    pub overall: Interval,
    /// Pooled win rate across only the decks whose cells produced a clean
    /// measurement.
    ///
    /// A cell polluted by engine-refused proposals did not end its games by
    /// play, so folding it into one aggregate quietly mixes a rules problem
    /// into a policy result. Reporting both makes the distinction visible
    /// instead of forcing a choice between a contaminated number and no number.
    pub clean: Interval,
}

impl LadderStep {
    /// Whether the challenger is a measured improvement.
    ///
    /// Requires the pooled interval to exclude 50% *and* no deck to have
    /// regressed significantly. The second condition is what stops a change
    /// that trades one archetype for another from being called progress.
    #[must_use]
    pub fn is_improvement(&self) -> bool {
        self.is_valid()
            && self.overall.point > 0.5
            && self.overall.excludes(0.5)
            && self.regressions().is_empty()
    }

    /// Whether the challenger is an improvement on the cells that measured
    /// cleanly.
    ///
    /// Weaker than [`Self::is_improvement`] and never a substitute for it: a
    /// contaminated cell is still unmeasured, and a change could in principle
    /// be responsible for the contamination. It exists so a known, documented
    /// engine defect in one deck does not make every other deck's evidence
    /// unreportable.
    #[must_use]
    pub fn is_clean_improvement(&self) -> bool {
        self.clean.samples > 0
            && self.clean.point > 0.5
            && self.clean.excludes(0.5)
            && self.regressions().is_empty()
    }

    /// Decks whose cells were polluted by engine-refused proposals.
    #[must_use]
    pub fn contaminated(&self) -> Vec<&DeckVerdict> {
        self.per_deck
            .iter()
            .filter(|verdict| verdict.rejected_moves > 0 || !verdict.engine_findings.is_empty())
            .collect()
    }

    /// Decks where the challenger is significantly worse.
    #[must_use]
    pub fn regressions(&self) -> Vec<&DeckVerdict> {
        self.per_deck
            .iter()
            .filter(|verdict| verdict.win_rate.point < 0.5 && verdict.win_rate.excludes(0.5))
            .collect()
    }

    /// Whether every cell produced a usable measurement.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.per_deck
            .iter()
            .all(|verdict| verdict.rejected_moves == 0 && verdict.engine_findings.is_empty())
    }

    #[must_use]
    pub fn rejected_moves(&self) -> u32 {
        self.per_deck
            .iter()
            .map(|verdict| verdict.rejected_moves)
            .sum()
    }

    /// The challenger's seat asymmetry pooled across every deck.
    #[must_use]
    pub fn seat_asymmetry(&self) -> Edge {
        let pool = |select: fn(&DeckVerdict) -> &Interval| {
            let wins = self.per_deck.iter().map(|deck| select(deck).wins).sum();
            let samples = self.per_deck.iter().map(|deck| select(deck).samples).sum();
            Interval::wilson(wins, samples)
        };
        difference(
            pool(|deck| &deck.win_rate_on_the_play),
            pool(|deck| &deck.win_rate_on_the_draw),
        )
    }

    /// Decks where the change moved seat-dependent behaviour.
    ///
    /// These are the cells where a 50% paired rate is uninformative rather than
    /// negative: something changed, and the pairing subtracted it out.
    #[must_use]
    pub fn seat_asymmetric(&self) -> Vec<&DeckVerdict> {
        self.per_deck
            .iter()
            .filter(|verdict| verdict.is_seat_asymmetric())
            .collect()
    }
}

/// Runs one version pair over every supplied deck.
///
/// # Errors
///
/// Propagates any setup failure from the match runner.
pub fn run_step(
    challenger: PolicyVersion,
    incumbent: PolicyVersion,
    decks: &[(String, Archetype)],
    seeds: u32,
    config: &DeckMatchConfig,
) -> Result<LadderStep, String> {
    let index = shared_card_index();
    let mut per_deck = Vec::new();
    let mut pooled_wins = 0;
    let mut pooled_games = 0;

    for (deck, archetype) in decks {
        let mut wins = 0;
        let mut decided = 0;
        // Indexed by the seat the challenger occupied: 0 on the play.
        let mut seat_wins = [0_u32; 2];
        let mut seat_decided = [0_u32; 2];
        let mut rejected = 0;
        let mut truncated = 0;
        let mut turns = 0_u64;
        let mut games = 0;
        let mut findings = Vec::new();

        for seed in 0..u64::from(seeds) {
            // Seat 0 is on the play. Swap which version sits there so the play
            // advantage cancels out of the comparison.
            for challenger_seat in 0_usize..2 {
                let versions = if challenger_seat == 0 {
                    [challenger, incumbent]
                } else {
                    [incumbent, challenger]
                };
                let result = run_versioned_matchup(
                    DeckMatchConfig {
                        shuffle_seed: seed,
                        ..*config
                    },
                    deck,
                    deck,
                    versions,
                    *archetype,
                    index.clone(),
                )?;
                games += 1;
                turns += u64::from(result.turns);
                rejected += result
                    .attempted_policy_moves
                    .saturating_sub(result.accepted_policy_moves);
                findings.extend(result.engine_findings.iter().cloned());
                match result.termination {
                    DeckMatchTermination::Winner(PlayerId(seat)) => {
                        decided += 1;
                        let challenger_won = u32::from(seat == challenger_seat);
                        wins += challenger_won;
                        seat_decided[challenger_seat] += 1;
                        seat_wins[challenger_seat] += challenger_won;
                    }
                    DeckMatchTermination::Draw => {}
                    _ => truncated += 1,
                }
            }
        }

        pooled_wins += wins;
        pooled_games += decided;
        per_deck.push(DeckVerdict {
            deck: deck.clone(),
            archetype: *archetype,
            win_rate: Interval::wilson(wins, decided),
            win_rate_on_the_play: Interval::wilson(seat_wins[0], seat_decided[0]),
            win_rate_on_the_draw: Interval::wilson(seat_wins[1], seat_decided[1]),
            games,
            mean_turns: turns_mean(turns, games),
            rejected_moves: rejected,
            truncated,
            engine_findings: findings,
        });
    }

    let clean_wins: u32 = per_deck
        .iter()
        .filter(|verdict| verdict.rejected_moves == 0 && verdict.engine_findings.is_empty())
        .map(|verdict| verdict.win_rate.wins)
        .sum();
    let clean_games: u32 = per_deck
        .iter()
        .filter(|verdict| verdict.rejected_moves == 0 && verdict.engine_findings.is_empty())
        .map(|verdict| verdict.win_rate.samples)
        .sum();
    Ok(LadderStep {
        challenger,
        incumbent,
        per_deck,
        overall: Interval::wilson(pooled_wins, pooled_games),
        clean: Interval::wilson(clean_wins, clean_games),
    })
}

/// Runs every successive version pair, oldest step first.
///
/// # Errors
///
/// Propagates any setup failure from the match runner.
pub fn run_ladder(
    decks: &[(String, Archetype)],
    seeds: u32,
    config: &DeckMatchConfig,
) -> Result<Vec<LadderStep>, String> {
    let mut steps = Vec::new();
    for challenger in PolicyVersion::ALL {
        let Some(incumbent) = challenger.previous() else {
            continue;
        };
        steps.push(run_step(challenger, incumbent, decks, seeds, config)?);
    }
    Ok(steps)
}

/// Mean turns, computed without a lossy `u64 as f64` on the accumulator.
fn turns_mean(total: u64, games: u32) -> f64 {
    let games = f64::from(games.max(1));
    let total = u32::try_from(total).map_or(f64::from(u32::MAX), f64::from);
    total / games
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(deck: &str, wins: u32, total: u32) -> DeckVerdict {
        // Split evenly across seats, which is the null the pairing assumes.
        seated_verdict(
            deck,
            wins / 2,
            total / 2,
            wins - wins / 2,
            total - total / 2,
        )
    }

    fn seated_verdict(
        deck: &str,
        play_wins: u32,
        play_total: u32,
        draw_wins: u32,
        draw_total: u32,
    ) -> DeckVerdict {
        DeckVerdict {
            deck: deck.to_owned(),
            archetype: Archetype::Aggro,
            win_rate: Interval::wilson(play_wins + draw_wins, play_total + draw_total),
            win_rate_on_the_play: Interval::wilson(play_wins, play_total),
            win_rate_on_the_draw: Interval::wilson(draw_wins, draw_total),
            games: play_total + draw_total,
            mean_turns: 15.0,
            rejected_moves: 0,
            truncated: 0,
            engine_findings: Vec::new(),
        }
    }

    /// The blind spot this reporting exists to close: a change that wins
    /// heavily on the play and loses just as heavily on the draw pools to
    /// exactly 50% and is indistinguishable from a no-op.
    #[test]
    fn a_seat_dependent_change_pools_to_no_change_and_is_reported_anyway() {
        let verdict = seated_verdict("a", 700, 1_000, 300, 1_000);
        assert!(
            (verdict.win_rate.point - 0.5).abs() < 1e-9,
            "the pairing must cancel it exactly, which is the problem"
        );
        assert!(
            !verdict.win_rate.excludes(0.5),
            "the paired rate reports no change"
        );
        let asymmetry = verdict.seat_asymmetry();
        assert!((asymmetry.point - 0.4).abs() < 1e-9);
        assert!(
            asymmetry.is_established(),
            "but the seat split resolves a 40-point effect"
        );
        assert!(verdict.is_seat_asymmetric());
    }

    /// The null case must stay null, or every rung reads as seat-asymmetric.
    #[test]
    fn an_evenly_split_change_reports_no_seat_asymmetry() {
        let verdict = verdict("a", 1_000, 2_000);
        assert!(!verdict.is_seat_asymmetric());
        assert!(!verdict.seat_asymmetry().is_established());
    }

    /// A small sample must not be able to claim a seat effect either.
    #[test]
    fn a_small_seat_split_claims_nothing() {
        let verdict = seated_verdict("a", 7, 12, 5, 12);
        assert!(
            !verdict.is_seat_asymmetric(),
            "24 games cannot resolve a 17-point seat split"
        );
    }

    fn step(per_deck: Vec<DeckVerdict>, wins: u32, total: u32) -> LadderStep {
        LadderStep {
            challenger: PolicyVersion::V2,
            incumbent: PolicyVersion::V1,
            per_deck,
            overall: Interval::wilson(wins, total),
            clean: Interval::wilson(wins, total),
        }
    }

    #[test]
    fn a_small_edge_is_not_an_improvement() {
        let ladder = step(vec![verdict("a", 55, 100)], 55, 100);
        assert!(
            !ladder.is_improvement(),
            "55% over 100 games does not exclude 50%"
        );
    }

    #[test]
    fn a_large_consistent_edge_is_an_improvement() {
        let ladder = step(
            vec![verdict("a", 700, 1_000), verdict("b", 650, 1_000)],
            1_350,
            2_000,
        );
        assert!(ladder.is_improvement());
        assert!(ladder.regressions().is_empty());
    }

    /// The condition that stops a change from trading one archetype for
    /// another and calling it progress.
    #[test]
    fn a_significant_regression_on_one_deck_blocks_the_step() {
        let ladder = step(
            vec![verdict("a", 900, 1_000), verdict("b", 300, 1_000)],
            1_200,
            2_000,
        );
        assert!(ladder.overall.point > 0.5);
        assert_eq!(ladder.regressions().len(), 1);
        assert!(
            !ladder.is_improvement(),
            "a deck that got significantly worse blocks the step"
        );
    }

    #[test]
    fn rejected_moves_invalidate_the_measurement() {
        let mut regressed = verdict("a", 900, 1_000);
        regressed.rejected_moves = 2;
        let ladder = step(vec![regressed], 900, 1_000);
        assert!(!ladder.is_valid());
        assert!(!ladder.is_improvement());
    }

    #[test]
    fn a_worse_challenger_is_never_an_improvement() {
        let ladder = step(vec![verdict("a", 100, 1_000)], 100, 1_000);
        assert!(!ladder.is_improvement());
    }
}
