//! Paired-seed matchup measurement with honest error bars.
//!
//! A win rate without a confidence interval is not a measurement, and Magic's
//! variance is large enough that acting on one is actively misleading: at 200
//! games a 55% result has a 95% interval of roughly +/-7 points, so a 52% deck
//! and a 58% deck are indistinguishable. Hillclimbing on that signal random
//! walks.
//!
//! Two variance controls are built in rather than optional:
//!
//! * **Paired seats.** Every seed is played twice, once with each deck on the
//!   play. A shuffle that hands one deck a perfect curve hands it to both
//!   sides, so most of the luck cancels.
//! * **Split play/draw reporting.** The play advantage is worth several points
//!   and would otherwise be silently mixed into every asymmetric matchup.
//!
//! Mirror matches are the calibration instrument: a deck against itself must
//! come out at 50% on paired seeds, and anything else is a seat bias or a
//! nondeterminism rather than a fact about the deck.

use crate::deck_match::{DeckMatchTermination, run_rav_deck_matchup};
use crate::{DeckMatchConfig, DeckMatchResult, EngineFinding};
use cardbench_magic_engine::PlayerId;
use std::collections::BTreeMap;

/// One game's measurement, reduced to what a campaign needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameRecord {
    pub shuffle_seed: u64,
    /// The deck seated as player zero, i.e. on the play.
    pub on_the_play: String,
    pub on_the_draw: String,
    /// Which seat the matchup's "deck A" occupied: 0 on the play, 1 on the
    /// draw. Recorded at generation time rather than inferred from deck ids,
    /// because in a mirror both ids are the same string and name-based
    /// attribution silently reports 100% for whichever side is checked first.
    pub deck_a_seat: usize,
    /// The winning seat, or `None` for a draw or a non-rules stop.
    pub winner_seat: Option<usize>,
    /// The winning deck id. `None` for a draw or a non-rules stop.
    pub winner: Option<String>,
    /// True when the game ended by the rules rather than by a bound.
    pub decisive: bool,
    pub termination: DeckMatchTermination,
    pub turns: u32,
    /// Final life, indexed the same way as `[on_the_play, on_the_draw]`.
    pub life: [i64; 2],
    pub accepted_policy_moves: u32,
    /// Proposals the engine refused. A healthy policy never produces one, so a
    /// nonzero total here is a policy-competence signal, not noise.
    pub rejected_policy_moves: u32,
    pub engine_findings: Vec<EngineFinding>,
    pub digest: String,
}

impl GameRecord {
    /// Whether deck A won this game. `None` when the game was not decisive.
    #[must_use]
    pub fn deck_a_won(&self) -> Option<bool> {
        self.winner_seat.map(|seat| seat == self.deck_a_seat)
    }

    /// Whether deck A was on the play.
    #[must_use]
    pub const fn deck_a_on_the_play(&self) -> bool {
        self.deck_a_seat == 0
    }

    fn from_result(result: &DeckMatchResult, deck_a_seat: usize) -> Self {
        let decisive = matches!(
            result.termination,
            DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
        );
        let winner_seat = result.winner.map(|PlayerId(seat)| seat);
        let winner = winner_seat.map(|seat| result.deck_ids[seat].clone());
        Self {
            shuffle_seed: result.config.shuffle_seed,
            on_the_play: result.deck_ids[0].clone(),
            on_the_draw: result.deck_ids[1].clone(),
            deck_a_seat,
            winner_seat,
            winner,
            decisive,
            termination: result.termination.clone(),
            turns: result.turns,
            life: result.life,
            accepted_policy_moves: result.accepted_policy_moves,
            rejected_policy_moves: result
                .attempted_policy_moves
                .saturating_sub(result.accepted_policy_moves),
            engine_findings: result.engine_findings.clone(),
            digest: result.digest.clone(),
        }
    }
}

/// A Wilson score interval for a binomial proportion.
///
/// Wilson rather than the normal approximation because the normal interval is
/// badly wrong near 0 and 1 and can extend outside `[0, 1]`, which is exactly
/// where a lopsided matchup lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub point: f64,
    pub low: f64,
    pub high: f64,
    pub samples: u32,
}

impl Interval {
    /// 95% Wilson interval for `wins` out of `total`.
    #[must_use]
    pub fn wilson(wins: u32, total: u32) -> Self {
        if total == 0 {
            return Self {
                point: 0.0,
                low: 0.0,
                high: 1.0,
                samples: 0,
            };
        }
        let z = 1.959_963_984_540_054_f64;
        let n = f64::from(total);
        let p = f64::from(wins) / n;
        let denominator = 1.0 + z * z / n;
        let centre = p + z * z / (2.0 * n);
        let spread = z * ((p * (1.0 - p) + z * z / (4.0 * n)) / n).sqrt();
        Self {
            point: p,
            low: ((centre - spread) / denominator).clamp(0.0, 1.0),
            high: ((centre + spread) / denominator).clamp(0.0, 1.0),
            samples: total,
        }
    }

    /// Width of the interval, i.e. how uncertain the measurement is.
    #[must_use]
    pub fn width(&self) -> f64 {
        self.high - self.low
    }

    /// Whether this proportion is distinguishable from `value` at 95%.
    #[must_use]
    pub fn excludes(&self, value: f64) -> bool {
        value < self.low || value > self.high
    }

    /// Whether two intervals are separated. Conservative: non-overlapping
    /// intervals imply a significant difference, though the converse does not
    /// hold, so this never claims significance it does not have.
    #[must_use]
    pub fn separated_from(&self, other: &Self) -> bool {
        self.high < other.low || other.high < self.low
    }
}

/// Diagnostics that explain *why* a matchup went the way it did.
///
/// Without these a hillclimb can only see that a change helped or hurt, never
/// which knob to turn next.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Diagnostics {
    pub games: u32,
    pub mean_turns: f64,
    /// Mean life remaining for the winner. A high number means the games are
    /// not close.
    pub mean_winner_life: f64,
    /// Mean life remaining for the loser. Below zero by a lot means overkill.
    pub mean_loser_life: f64,
    pub mean_accepted_moves: f64,
    /// Total refused proposals across the sample. Must be zero.
    pub rejected_moves: u32,
    /// Games that ended by a move or turn bound rather than by the rules.
    pub truncated: u32,
    pub draws: u32,
}

/// One ordered pairing's result: deck A against deck B, both seats.
#[derive(Clone, Debug)]
pub struct Matchup {
    pub deck_a: String,
    pub deck_b: String,
    /// Deck A's win rate over all decisive games, both seats.
    pub overall: Interval,
    /// Deck A's win rate in the games where it was on the play.
    pub on_the_play: Interval,
    /// Deck A's win rate in the games where it was on the draw.
    pub on_the_draw: Interval,
    pub diagnostics: Diagnostics,
    pub games: Vec<GameRecord>,
    pub engine_findings: Vec<EngineFinding>,
}

impl Matchup {
    /// True when this is a deck against itself.
    #[must_use]
    pub fn is_mirror(&self) -> bool {
        self.deck_a == self.deck_b
    }

    /// The play advantage this matchup measured, in win-rate points.
    #[must_use]
    pub fn play_advantage(&self) -> f64 {
        self.on_the_play.point - self.on_the_draw.point
    }

    /// A mirror must land on 50%. Anything else is a seat bias or a
    /// nondeterminism in the harness, not a fact about the deck, so this is a
    /// free correctness check on every campaign.
    #[must_use]
    pub fn mirror_is_calibrated(&self) -> bool {
        !self.is_mirror() || !self.overall.excludes(0.5)
    }
}

/// Runs one ordered pairing over paired seeds.
///
/// Each seed is played twice with the seats swapped, so the same shuffle
/// benefits each deck once. `seeds` is the number of *pairs*; the game count is
/// twice that.
///
/// # Errors
///
/// Returns an error when a deck id is unknown or the runner cannot set up a
/// game. An ordinary policy rejection is data, not an error.
pub fn measure_matchup(
    deck_a: &str,
    deck_b: &str,
    seeds: impl IntoIterator<Item = u64>,
    config: &DeckMatchConfig,
) -> Result<Matchup, String> {
    let mut games = Vec::new();
    for seed in seeds {
        // Deck A takes seat 0 in the first game of the pair and seat 1 in the
        // second, so the same shuffle is played from both sides.
        for (deck_a_seat, (first, second)) in
            [(deck_a, deck_b), (deck_b, deck_a)].into_iter().enumerate()
        {
            let result = run_rav_deck_matchup(
                DeckMatchConfig {
                    shuffle_seed: seed,
                    ..config.clone()
                },
                first,
                second,
            )?;
            games.push(GameRecord::from_result(&result, deck_a_seat));
        }
    }
    Ok(summarize(deck_a, deck_b, games))
}

fn summarize(deck_a: &str, deck_b: &str, games: Vec<GameRecord>) -> Matchup {
    let mut wins = 0;
    let mut decided = 0;
    let mut play_wins = 0;
    let mut play_decided = 0;
    let mut draw_wins = 0;
    let mut draw_decided = 0;
    let mut diagnostics = Diagnostics::default();
    let mut turns_total = 0_u64;
    let mut winner_life = 0_i64;
    let mut loser_life = 0_i64;
    let mut moves_total = 0_u64;
    let mut engine_findings = Vec::new();

    for game in &games {
        diagnostics.games += 1;
        turns_total += u64::from(game.turns);
        moves_total += u64::from(game.accepted_policy_moves);
        diagnostics.rejected_moves += game.rejected_policy_moves;
        engine_findings.extend(game.engine_findings.iter().cloned());
        if !game.decisive {
            diagnostics.truncated += 1;
            continue;
        }
        let Some(a_wins) = game.deck_a_won() else {
            diagnostics.draws += 1;
            continue;
        };
        decided += 1;
        wins += u32::from(a_wins);
        if game.deck_a_on_the_play() {
            play_decided += 1;
            play_wins += u32::from(a_wins);
        } else {
            draw_decided += 1;
            draw_wins += u32::from(a_wins);
        }
        // Winner and loser life, resolved by seat rather than by deck name so
        // a mirror is attributed correctly.
        if let Some(seat) = game.winner_seat {
            winner_life += game.life[seat];
            loser_life += game.life[1 - seat];
        }
    }

    let n = f64::from(diagnostics.games.max(1));
    diagnostics.mean_turns = turns_total as f64 / n;
    diagnostics.mean_accepted_moves = moves_total as f64 / n;
    let d = f64::from(decided.max(1));
    diagnostics.mean_winner_life = winner_life as f64 / d;
    diagnostics.mean_loser_life = loser_life as f64 / d;

    Matchup {
        deck_a: deck_a.to_owned(),
        deck_b: deck_b.to_owned(),
        overall: Interval::wilson(wins, decided),
        on_the_play: Interval::wilson(play_wins, play_decided),
        on_the_draw: Interval::wilson(draw_wins, draw_decided),
        diagnostics,
        games,
        engine_findings,
    }
}

/// A full round-robin including mirrors.
#[derive(Clone, Debug)]
pub struct Matrix {
    pub decks: Vec<String>,
    pub matchups: Vec<Matchup>,
}

impl Matrix {
    #[must_use]
    pub fn get(&self, deck_a: &str, deck_b: &str) -> Option<&Matchup> {
        self.matchups
            .iter()
            .find(|matchup| matchup.deck_a == deck_a && matchup.deck_b == deck_b)
    }

    /// Each deck's win rate across every non-mirror pairing.
    ///
    /// Both sides of each pairing are counted. Counting only `deck_a` would
    /// silently omit any deck that never sorts first -- with three decks, the
    /// last one would be missing from the standings entirely.
    #[must_use]
    pub fn standings(&self) -> Vec<(String, Interval)> {
        let mut wins: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
        for matchup in &self.matchups {
            if matchup.is_mirror() {
                continue;
            }
            let decided = matchup.overall.samples;
            let a_won = (matchup.overall.point * f64::from(decided)).round() as u32;
            let entry = wins.entry(matchup.deck_a.as_str()).or_default();
            entry.0 += a_won;
            entry.1 += decided;
            let entry = wins.entry(matchup.deck_b.as_str()).or_default();
            entry.0 += decided - a_won;
            entry.1 += decided;
        }
        let mut standings: Vec<(String, Interval)> = wins
            .into_iter()
            .map(|(deck, (won, total))| (deck.to_owned(), Interval::wilson(won, total)))
            .collect();
        standings.sort_by(|left, right| {
            right
                .1
                .point
                .total_cmp(&left.1.point)
                .then(left.0.cmp(&right.0))
        });
        standings
    }

    /// Every mirror that failed its calibration check.
    #[must_use]
    pub fn miscalibrated_mirrors(&self) -> Vec<&Matchup> {
        self.matchups
            .iter()
            .filter(|matchup| !matchup.mirror_is_calibrated())
            .collect()
    }

    /// Every engine finding surfaced anywhere in the campaign.
    #[must_use]
    pub fn engine_findings(&self) -> Vec<&EngineFinding> {
        self.matchups
            .iter()
            .flat_map(|matchup| matchup.engine_findings.iter())
            .collect()
    }

    /// Total refused proposals. A healthy campaign reports zero.
    #[must_use]
    pub fn rejected_moves(&self) -> u32 {
        self.matchups
            .iter()
            .map(|matchup| matchup.diagnostics.rejected_moves)
            .sum()
    }
}

/// Runs every unordered pairing plus every mirror.
///
/// Unordered because [`measure_matchup`] already plays both seats: running
/// both orders as well would double the cost for no extra information.
///
/// # Errors
///
/// Propagates any setup failure from the underlying runner.
pub fn measure_matrix(
    decks: &[String],
    seeds: u32,
    config: &DeckMatchConfig,
) -> Result<Matrix, String> {
    let seed_range: Vec<u64> = (0..u64::from(seeds)).collect();
    let mut matchups = Vec::new();
    for (index, deck_a) in decks.iter().enumerate() {
        for deck_b in decks.iter().skip(index) {
            matchups.push(measure_matchup(deck_a, deck_b, seed_range.clone(), config)?);
        }
    }
    Ok(Matrix {
        decks: decks.to_vec(),
        matchups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wilson_interval_brackets_its_point_estimate() {
        let interval = Interval::wilson(55, 100);
        assert!((interval.point - 0.55).abs() < 1e-9);
        assert!(interval.low < 0.55 && interval.high > 0.55);
        assert!(interval.low >= 0.0 && interval.high <= 1.0);
    }

    /// The reason Wilson is used: the normal approximation puts the bound
    /// outside `[0, 1]` at the extremes, which is where lopsided matchups land.
    #[test]
    fn a_wilson_interval_stays_inside_the_unit_range_at_the_extremes() {
        for (wins, total) in [(0, 20), (20, 20), (1, 200), (199, 200)] {
            let interval = Interval::wilson(wins, total);
            assert!(
                interval.low >= 0.0 && interval.high <= 1.0,
                "{wins}/{total} produced {interval:?}"
            );
        }
    }

    /// The honesty check this whole module exists for: a small sample must not
    /// be able to claim a real edge.
    #[test]
    fn a_small_sample_cannot_distinguish_a_real_edge() {
        let small = Interval::wilson(11, 20);
        assert!(
            !small.excludes(0.5),
            "20 games must not resolve a 55% win rate"
        );
        let large = Interval::wilson(1_100, 2_000);
        assert!(
            large.excludes(0.5),
            "2000 games must resolve the same 55% win rate"
        );
    }

    #[test]
    fn separation_is_conservative() {
        let left = Interval::wilson(60, 100);
        let right = Interval::wilson(58, 100);
        assert!(
            !left.separated_from(&right),
            "overlapping intervals must not be called separated"
        );
        let strong = Interval::wilson(900, 1_000);
        let weak = Interval::wilson(100, 1_000);
        assert!(strong.separated_from(&weak));
    }

    #[test]
    fn an_empty_sample_claims_nothing() {
        let empty = Interval::wilson(0, 0);
        assert_eq!(empty.samples, 0);
        assert!(!empty.excludes(0.5), "no data must exclude nothing");
        assert!((empty.width() - 1.0).abs() < 1e-9);
    }
}
