//! Replayable Elo ratings for paired-seed development matches.
//!
//! Ratings are deliberately a progress ledger, not a replacement for the
//! ladder's confidence intervals. A record represents both seats of one
//! shuffle seed, so play/draw effects are cancelled before the Elo update.
//! Rejected, incomplete, or engine-finding games never enter the ledger.

use crate::matchup::{GameRecord, Matchup};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

/// Starting point for a new player or deck.
pub const DEFAULT_RATING: f64 = 1_500.0;
/// Standard Elo K-factor used for one paired-seed match.
pub const ELO_K_FACTOR: f64 = 32.0;
const LEDGER_HEADER: &str = "# cardbench.magic.elo.v1";

/// The result of one paired seed, stored as half-points out of four.
///
/// Each of the two games contributes two half-points: a win contributes two,
/// a draw one, and a loss zero. Thus 4 is a 2-0 result, 2 is 1-1, and 0 is
/// 0-2. The representation also handles a rules-valid draw in one game.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EloMatchRecord {
    pub axis: String,
    pub left: String,
    pub right: String,
    pub shuffle_seed: u64,
    pub score_half: u8,
}

impl EloMatchRecord {
    /// Creates a paired-seed record.
    pub fn new(
        axis: impl Into<String>,
        left: impl Into<String>,
        right: impl Into<String>,
        shuffle_seed: u64,
        score_half: u8,
    ) -> Result<Self, String> {
        if score_half > 4 {
            return Err(format!(
                "Elo score must be between 0 and 4, got {score_half}"
            ));
        }
        let axis = axis.into();
        let left = left.into();
        let right = right.into();
        if axis.is_empty() || left.is_empty() || right.is_empty() {
            return Err("Elo records require non-empty axis and player ids".to_owned());
        }
        if [axis.as_str(), left.as_str(), right.as_str()]
            .iter()
            .any(|value| value.contains('\t') || value.contains('\n'))
        {
            return Err("Elo ids cannot contain tabs or newlines".to_owned());
        }
        Ok(Self {
            axis,
            left,
            right,
            shuffle_seed,
            score_half,
        })
    }

    fn key(&self) -> (&str, &str, &str, u64) {
        (&self.axis, &self.left, &self.right, self.shuffle_seed)
    }
}

#[derive(Clone, Debug)]
struct EloAnchor {
    axis: String,
    id: String,
    rating: f64,
}

/// One current rating as reconstructed from the ledger.
#[derive(Clone, Debug, PartialEq)]
pub struct RatingStanding {
    pub id: String,
    pub rating: f64,
    pub matches: u32,
    pub anchored: bool,
}

/// Append-only Elo input with fixed reference anchors.
///
/// The on-disk format is intentionally small and dependency-free:
///
/// ```text
/// # cardbench.magic.elo.v1
/// anchor<TAB>deck<TAB>rav_boros_aggro<TAB>1500
/// match<TAB>deck<TAB>candidate<TAB>rav_boros_aggro<TAB>0<TAB>4
/// ```
///
/// Ratings are always replayed from the records. This makes the file
/// inspectable, reproducible, and safe to extend without trusting a cached
/// floating-point total.
#[derive(Clone, Debug, Default)]
pub struct EloLedger {
    anchors: Vec<EloAnchor>,
    matches: Vec<EloMatchRecord>,
}

impl EloLedger {
    /// Loads a ledger, treating a missing file as an empty ledger.
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(format!(
                    "failed to read Elo ledger {}: {error}",
                    path.display()
                ));
            }
        };
        let mut ledger = Self::default();
        for (line_number, line) in contents.lines().enumerate() {
            let line_number = line_number + 1;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.first().copied() {
                Some("anchor") if fields.len() == 4 => {
                    let rating = fields[3].parse::<f64>().map_err(|error| {
                        format!("invalid Elo anchor rating on line {line_number}: {error}")
                    })?;
                    ledger.ensure_anchor(fields[1], fields[2], rating)?;
                }
                Some("match") if fields.len() == 6 => {
                    let shuffle_seed = fields[4].parse::<u64>().map_err(|error| {
                        format!("invalid Elo shuffle seed on line {line_number}: {error}")
                    })?;
                    let score_half = fields[5].parse::<u8>().map_err(|error| {
                        format!("invalid Elo score on line {line_number}: {error}")
                    })?;
                    let record = EloMatchRecord::new(
                        fields[1],
                        fields[2],
                        fields[3],
                        shuffle_seed,
                        score_half,
                    )
                    .map_err(|error| {
                        format!("invalid Elo record on line {line_number}: {error}")
                    })?;
                    ledger.record_match(record).map_err(|error| {
                        format!("invalid Elo record on line {line_number}: {error}")
                    })?;
                }
                _ => {
                    return Err(format!(
                        "invalid Elo ledger line {line_number}: expected anchor or match record"
                    ));
                }
            }
        }
        Ok(ledger)
    }

    /// Saves the complete replayable ledger.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut contents = String::new();
        contents.push_str(LEDGER_HEADER);
        contents.push('\n');
        for anchor in &self.anchors {
            writeln!(
                contents,
                "anchor\t{}\t{}\t{}",
                anchor.axis, anchor.id, anchor.rating
            )
            .expect("writing to a String cannot fail");
        }
        for record in &self.matches {
            writeln!(
                contents,
                "match\t{}\t{}\t{}\t{}\t{}",
                record.axis, record.left, record.right, record.shuffle_seed, record.score_half
            )
            .expect("writing to a String cannot fail");
        }
        fs::write(path, contents)
            .map_err(|error| format!("failed to write Elo ledger {}: {error}", path.display()))
    }

    /// Adds or validates a fixed reference rating.
    pub fn ensure_anchor(
        &mut self,
        axis: impl Into<String>,
        id: impl Into<String>,
        rating: f64,
    ) -> Result<(), String> {
        if !rating.is_finite() {
            return Err("Elo anchor rating must be finite".to_owned());
        }
        let axis = axis.into();
        let id = id.into();
        if axis.is_empty() || id.is_empty() {
            return Err("Elo anchors require non-empty axis and id".to_owned());
        }
        if let Some(existing) = self
            .anchors
            .iter()
            .find(|anchor| anchor.axis == axis && anchor.id == id)
        {
            if (existing.rating - rating).abs() > f64::EPSILON {
                return Err(format!(
                    "Elo anchor {axis}/{id} already has rating {}, not {rating}",
                    existing.rating
                ));
            }
            return Ok(());
        }
        self.anchors.push(EloAnchor { axis, id, rating });
        Ok(())
    }

    /// Appends a record unless its axis, players, and seed already exist.
    /// Returns whether the record was newly added.
    pub fn record_match(&mut self, record: EloMatchRecord) -> Result<bool, String> {
        if self
            .matches
            .iter()
            .any(|existing| existing.key() == record.key())
        {
            return Ok(false);
        }
        self.matches.push(record);
        Ok(true)
    }

    /// Adds every clean paired seed in a measured matchup.
    ///
    /// The matchup must contain exactly two games for each seed. A pair with a
    /// rejected proposal, engine finding, or non-terminal stop is ignored in
    /// full, so the rating never rewards an invalid game.
    pub fn record_matchup(&mut self, axis: &str, matchup: &Matchup) -> Result<usize, String> {
        if !matchup.engine_findings.is_empty() {
            return Ok(0);
        }
        let mut by_seed: BTreeMap<u64, Vec<&GameRecord>> = BTreeMap::new();
        for game in &matchup.games {
            by_seed.entry(game.shuffle_seed).or_default().push(game);
        }
        let mut added = 0;
        for (shuffle_seed, games) in by_seed {
            if games.len() != 2 {
                return Err(format!(
                    "Elo matchup {} vs {} has {} games for seed {shuffle_seed}, expected 2",
                    matchup.deck_a,
                    matchup.deck_b,
                    games.len()
                ));
            }
            if games.iter().any(|game| {
                !game.decisive || game.rejected_policy_moves > 0 || !game.engine_findings.is_empty()
            }) {
                continue;
            }
            let score_half = games
                .iter()
                .map(|game| match game.winner_seat {
                    Some(seat) if seat == game.deck_a_seat => 2,
                    Some(_) => 0,
                    None => 1,
                })
                .sum();
            let record = EloMatchRecord::new(
                axis,
                &matchup.deck_a,
                &matchup.deck_b,
                shuffle_seed,
                score_half,
            )?;
            if self.record_match(record)? {
                added += 1;
            }
        }
        Ok(added)
    }

    /// Returns standings for one axis, replayed from the anchors and records.
    #[must_use]
    pub fn standings(&self, axis: &str) -> Vec<RatingStanding> {
        let anchor_ratings: BTreeMap<String, f64> = self
            .anchors
            .iter()
            .filter(|anchor| anchor.axis == axis)
            .map(|anchor| (anchor.id.clone(), anchor.rating))
            .collect();
        let anchor_ids: BTreeSet<String> = anchor_ratings.keys().cloned().collect();
        let mut states: BTreeMap<String, RatingState> = anchor_ratings
            .iter()
            .map(|(id, rating)| {
                (
                    id.clone(),
                    RatingState {
                        rating: *rating,
                        matches: 0,
                    },
                )
            })
            .collect();

        // Elo is inherently sequential, so make replay order explicit rather
        // than letting the order in which candidates happened to be run decide
        // the leaderboard. This also makes adding a second candidate's batch
        // unable to change the first candidate's path through its own results.
        let mut records: Vec<&EloMatchRecord> = self
            .matches
            .iter()
            .filter(|record| record.axis == axis)
            .collect();
        records.sort_by(|left, right| {
            left.shuffle_seed
                .cmp(&right.shuffle_seed)
                .then_with(|| left.left.cmp(&right.left))
                .then_with(|| left.right.cmp(&right.right))
        });

        for record in records {
            let left_rating = states
                .get(&record.left)
                .map_or(DEFAULT_RATING, |state| state.rating);
            let right_rating = states
                .get(&record.right)
                .map_or(DEFAULT_RATING, |state| state.rating);
            let expected_left = expected_score(left_rating, right_rating);
            let actual_left = f64::from(record.score_half) / 4.0;
            let left_delta = if anchor_ids.contains(&record.left) {
                0.0
            } else {
                ELO_K_FACTOR * (actual_left - expected_left)
            };
            let right_delta = if anchor_ids.contains(&record.right) {
                0.0
            } else {
                ELO_K_FACTOR * ((1.0 - actual_left) - (1.0 - expected_left))
            };

            let left = states.entry(record.left.clone()).or_default();
            left.rating += left_delta;
            left.matches += 1;
            let right = states.entry(record.right.clone()).or_default();
            right.rating += right_delta;
            right.matches += 1;
        }

        let mut standings: Vec<_> = states
            .into_iter()
            .map(|(id, state)| RatingStanding {
                anchored: anchor_ids.contains(&id),
                id,
                rating: state.rating,
                matches: state.matches,
            })
            .collect();
        standings.sort_by(|left, right| {
            right
                .rating
                .total_cmp(&left.rating)
                .then_with(|| left.id.cmp(&right.id))
        });
        standings
    }

    /// Number of stored paired-seed records.
    #[must_use]
    pub fn record_count(&self) -> usize {
        self.matches.len()
    }

    /// Returns every axis represented in the ledger, in stable order.
    #[must_use]
    pub fn axes(&self) -> Vec<String> {
        let mut axes = BTreeSet::new();
        axes.extend(self.anchors.iter().map(|anchor| anchor.axis.clone()));
        axes.extend(self.matches.iter().map(|record| record.axis.clone()));
        axes.into_iter().collect()
    }
}

#[derive(Clone, Copy, Debug)]
struct RatingState {
    rating: f64,
    matches: u32,
}

impl Default for RatingState {
    fn default() -> Self {
        Self {
            rating: DEFAULT_RATING,
            matches: 0,
        }
    }
}

/// Expected score for the first player under the standard Elo curve.
#[must_use]
pub fn expected_score(rating: f64, opponent_rating: f64) -> f64 {
    1.0 / (1.0 + 10.0_f64.powf((opponent_rating - rating) / 400.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_ratings_expect_a_draw() {
        assert!((expected_score(DEFAULT_RATING, DEFAULT_RATING) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn anchored_rating_moves_only_the_candidate_and_deduplicates_seeds() {
        let mut ledger = EloLedger::default();
        ledger
            .ensure_anchor("deck", "control", DEFAULT_RATING)
            .unwrap();
        let record = EloMatchRecord::new("deck", "candidate", "control", 3, 4).unwrap();
        assert!(ledger.record_match(record.clone()).unwrap());
        assert!(!ledger.record_match(record).unwrap());

        let standings = ledger.standings("deck");
        let candidate = standings
            .iter()
            .find(|entry| entry.id == "candidate")
            .unwrap();
        let control = standings
            .iter()
            .find(|entry| entry.id == "control")
            .unwrap();
        assert!(candidate.rating > DEFAULT_RATING);
        assert!((control.rating - DEFAULT_RATING).abs() < f64::EPSILON);
        assert_eq!(candidate.matches, 1);
        assert_eq!(control.matches, 1);
    }

    #[test]
    fn ledger_round_trips_through_the_text_format() {
        let path = std::env::temp_dir().join(format!(
            "cardbench-magic-elo-{}-{}.tsv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut ledger = EloLedger::default();
        ledger
            .ensure_anchor("deck", "control", DEFAULT_RATING)
            .unwrap();
        ledger
            .record_match(EloMatchRecord::new("deck", "candidate", "control", 0, 2).unwrap())
            .unwrap();
        ledger.save(&path).unwrap();
        let loaded = EloLedger::load(&path).unwrap();
        assert_eq!(loaded.record_count(), 1);
        assert_eq!(loaded.standings("deck"), ledger.standings("deck"));
        std::fs::remove_file(path).unwrap();
    }
}
