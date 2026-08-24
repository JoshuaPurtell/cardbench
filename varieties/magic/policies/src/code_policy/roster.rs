//! The fixed evaluation surface for `cardbench/magic/code_policy`.
//!
//! A *roster* names the two opponent splits and the frozen reference. It is the
//! only place the surface is defined, and both the train and the held-out sweep
//! resolve through it, so the two cannot drift apart.
//!
//! ```text
//! train    5 opponents, visible in the workspace. Feedback only.
//! heldout  5 opponents, sealed under `.sealed/code_policy/`. Authority.
//! ```
//!
//! # Cells
//!
//! ```text
//! cell = (candidate deck, candidate pilot, opponent, opponent deck, seat, seed)
//! ```
//!
//! Two identifiers come off every cell and they do different jobs:
//!
//! * [`Cell::cell_id`] is the full coordinate. It is what coverage is checked
//!   against, per arm, and it carries the candidate's own deck and pilot.
//! * [`Cell::pair_key`] drops the candidate's half. It is the coordinate the
//!   candidate arm and the reference arm *share*, and it is what the paired
//!   comparison joins on.
//!
//! They have to be separate because this is a deck-and-pilot task. The candidate
//! submits both artifacts, so the candidate deck is not a shared axis: the
//! reference arm plays the roster's fixed `reference.deck` and the candidate arm
//! plays whatever the candidate brought. Joining on the full cell id would pair
//! nothing; joining on the shared coordinate pairs exactly the opponent, the
//! opponent's deck, the seat and the shuffle seed, which are the only things
//! both arms have in common and the only things the pairing is meant to cancel.
//!
//! # Seats
//!
//! Every (opponent, opponent deck, seed) is played twice, once with the
//! candidate on the play and once on the draw, so the play advantage cancels —
//! the same discipline `ladder.rs` applies, for the same reason.
//!
//! # Sealing
//!
//! The held-out block in the committed roster carries **counts and a digest,
//! nothing else**. Which pilots, on which decks, in which order, is in the
//! sealed manifest and not in this repository. See `.sealed/README.md` for what
//! sealing does and does not cover.

use crate::archetype::Archetype;
use crate::archetypes::PolicyVersion;
use crate::code_policy::sha256;
use cardbench_magic_rav::{DeckFixture, load_constructed_decks, load_hillclimb_decks};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Schema of the committed roster file.
pub const ROSTER_SCHEMA: &str = "cardbench.magic.code_policy_roster.v1";
/// Schema of the sealed held-out manifest.
pub const HELDOUT_SCHEMA: &str = "cardbench.magic.code_policy_heldout.v1";

/// Which opponent set a sweep is scored over.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Split {
    /// Visible opponents. Feedback only — never the reported score.
    Train,
    /// Sealed opponents. The authority the reward is computed from.
    Heldout,
}

impl Split {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Heldout => "heldout",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "train" => Some(Self::Train),
            "heldout" => Some(Self::Heldout),
            _ => None,
        }
    }
}

/// Which seat the candidate occupies. Seat zero is on the play.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Seat {
    Play,
    Draw,
}

impl Seat {
    pub const ALL: [Self; 2] = [Self::Play, Self::Draw];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Play => "p0",
            Self::Draw => "p1",
        }
    }

    /// Index of the seat the candidate sits in. The opponent takes the other.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Play => 0,
            Self::Draw => 1,
        }
    }
}

/// One opponent: a pilot generation seated on a specific deck.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct OpponentSpec {
    pub id: String,
    /// Archetype-policy generation id, e.g. `v3`.
    pub pilot: String,
    /// Deck fixture id the opponent pilots.
    pub deck: String,
}

/// The frozen origin every score is a delta against.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ReferenceSpec {
    pub id: String,
    pub pilot: String,
    pub deck: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TrainBlock {
    opponents: Vec<OpponentSpec>,
    seeds: u32,
    cell_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
struct HeldoutBlock {
    manifest: String,
    sha256: String,
    opponent_count: usize,
    seeds: u32,
    cell_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
struct RosterFile {
    schema_version: String,
    roster_id: String,
    task_id: String,
    reference: ReferenceSpec,
    candidate_deck_pool: Vec<String>,
    train: TrainBlock,
    heldout: HeldoutBlock,
    score_metric: String,
}

/// The sealed held-out manifest, read only when a held-out sweep is asked for.
#[derive(Clone, Debug, Deserialize)]
struct HeldoutManifest {
    schema_version: String,
    split: String,
    opponents: Vec<OpponentSpec>,
    seeds: u32,
    cell_count: usize,
}

/// One evaluation coordinate.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Cell {
    pub candidate_deck: String,
    pub candidate_pilot: String,
    pub opponent_id: String,
    pub opponent_deck: String,
    pub seat: Seat,
    pub seed: u64,
}

impl Cell {
    /// The full coordinate, including the candidate's half. Coverage is checked
    /// against the set of these.
    #[must_use]
    pub fn cell_id(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.candidate_deck,
            self.candidate_pilot,
            self.opponent_id,
            self.opponent_deck,
            self.seat.id(),
            self.seed
        )
    }

    /// The coordinate shared with the reference arm. The paired comparison
    /// joins on this and on nothing else.
    #[must_use]
    pub fn pair_key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.opponent_id,
            self.opponent_deck,
            self.seat.id(),
            self.seed
        )
    }
}

/// One arm of the comparison: a deck plus the pilot generation flying it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entrant {
    pub label: String,
    pub deck: String,
    pub pilot: PolicyVersion,
    /// Resolved from the deck fixture, never declared twice.
    pub archetype: Archetype,
}

/// One opponent, resolved against the deck fixtures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Opponent {
    pub id: String,
    pub deck: String,
    pub pilot: PolicyVersion,
    pub archetype: Archetype,
}

/// A roster resolved into the concrete surface a sweep must cover.
#[derive(Clone, Debug)]
pub struct Surface {
    pub split: Split,
    pub roster_id: String,
    pub task_id: String,
    pub score_metric: String,
    /// `Some` for held-out: the digest the manifest actually hashed to, having
    /// already been checked against the committed pin.
    pub manifest_sha256: Option<String>,
    pub reference: Entrant,
    pub opponents: Vec<Opponent>,
    pub seeds: u32,
    /// Deck ids a candidate is allowed to submit. Kept in the roster so a
    /// candidate cannot quietly widen its own pool.
    pub candidate_deck_pool: Vec<String>,
}

impl Surface {
    /// Every cell one arm must cover, in canonical order.
    ///
    /// Canonical order is authority: the coverage check and the paired join both
    /// read it, so a sweep that enumerates differently is a sweep that scores
    /// something else.
    #[must_use]
    pub fn cells(&self, entrant: &Entrant) -> Vec<Cell> {
        let mut cells = Vec::new();
        for opponent in &self.opponents {
            for seat in Seat::ALL {
                for seed in 0..u64::from(self.seeds) {
                    cells.push(Cell {
                        candidate_deck: entrant.deck.clone(),
                        candidate_pilot: entrant.pilot.id().to_owned(),
                        opponent_id: opponent.id.clone(),
                        opponent_deck: opponent.deck.clone(),
                        seat,
                        seed,
                    });
                }
            }
        }
        cells
    }

    /// The cell count one arm must produce.
    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.opponents.len() * Seat::ALL.len() * self.seeds as usize
    }
}

/// Every deck fixture a roster may name: the constructed control group and the
/// hillclimb candidate index.
///
/// The rules-coverage fixtures in `reference_decks.toml` are deliberately not
/// here. They run 44 to 52 lands in a sixty-card list across four to seven
/// distinct cards; they exist to exercise the rules engine, not to be played.
/// An opponent surface built out of them would measure who can beat a deck that
/// cannot function, which is not the thing this benchmark claims to measure.
fn playable_fixtures() -> Result<Vec<DeckFixture>, String> {
    let mut decks = load_constructed_decks().map_err(|error| error.to_string())?;
    decks.extend(load_hillclimb_decks().map_err(|error| error.to_string())?);
    Ok(decks)
}

fn archetype_of(decks: &[DeckFixture], id: &str) -> Result<Archetype, String> {
    let fixture = decks
        .iter()
        .find(|deck| deck.id == id)
        .ok_or_else(|| format!("roster names unknown deck fixture `{id}`"))?;
    if fixture.archetype.is_empty() {
        return Err(format!(
            "deck `{id}` declares no archetype, so no archetype pilot can fly it"
        ));
    }
    Archetype::parse(&fixture.archetype)
        .ok_or_else(|| format!("deck `{id}` names unknown archetype `{}`", fixture.archetype))
}

fn pilot_of(value: &str, context: &str) -> Result<PolicyVersion, String> {
    PolicyVersion::parse(value).ok_or_else(|| {
        let known: Vec<&str> = PolicyVersion::ALL.iter().map(|version| version.id()).collect();
        format!("{context} names unknown pilot `{value}`; known: {}", known.join(" "))
    })
}

fn resolve_opponents(
    specs: &[OpponentSpec],
    decks: &[DeckFixture],
) -> Result<Vec<Opponent>, String> {
    let mut seen = BTreeSet::new();
    let mut opponents = Vec::new();
    for spec in specs {
        if !seen.insert(spec.id.clone()) {
            return Err(format!("duplicate opponent id `{}`", spec.id));
        }
        opponents.push(Opponent {
            id: spec.id.clone(),
            deck: spec.deck.clone(),
            pilot: pilot_of(&spec.pilot, &format!("opponent `{}`", spec.id))?,
            archetype: archetype_of(decks, &spec.deck)?,
        });
    }
    if opponents.is_empty() {
        return Err("roster declares no opponents".to_owned());
    }
    Ok(opponents)
}

/// Canonical JSON — sorted keys, no whitespace.
///
/// Byte-identical to Python's
/// `json.dumps(obj, sort_keys=True, separators=(",", ":"))`, which is what the
/// build script hashes, because `serde_json::Value` orders object keys and the
/// compact writer emits the same separators. It holds only for ASCII content;
/// Python escapes non-ASCII by default and `serde_json` does not, so the sealed
/// manifest is required to be ASCII and the builder enforces it.
fn canonical_json(value: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("cannot canonicalise manifest: {error}"))
}

/// Loads a roster and resolves one split into a concrete surface.
///
/// # Errors
///
/// Returns an error when the roster is unreadable or malformed, when it names a
/// deck or pilot that does not exist, or — for the held-out split — when the
/// sealed manifest is absent, does not match its committed digest, or disagrees
/// with the counts published in the roster.
#[allow(clippy::too_many_lines)] // Every branch is a distinct fail-closed check; splitting them hides the list.
pub fn load_surface(roster_path: &Path, split: Split) -> Result<Surface, String> {
    let text = std::fs::read_to_string(roster_path)
        .map_err(|error| format!("cannot read roster {}: {error}", roster_path.display()))?;
    let roster: RosterFile = serde_json::from_str(&text)
        .map_err(|error| format!("cannot parse roster {}: {error}", roster_path.display()))?;
    if roster.schema_version != ROSTER_SCHEMA {
        return Err(format!(
            "roster schema `{}` is not `{ROSTER_SCHEMA}`",
            roster.schema_version
        ));
    }

    let decks = playable_fixtures()?;
    let reference = Entrant {
        label: roster.reference.id.clone(),
        deck: roster.reference.deck.clone(),
        pilot: pilot_of(&roster.reference.pilot, "reference")?,
        archetype: archetype_of(&decks, &roster.reference.deck)?,
    };
    if reference.label != crate::code_policy::reference::REFERENCE_ID
        || reference.pilot != crate::code_policy::reference::REFERENCE_VERSION
    {
        return Err(format!(
            "roster reference `{}`/`{}` does not match the frozen reference `{}`/`{}`; \
             the ranking origin is defined in code, not in the roster",
            reference.label,
            reference.pilot,
            crate::code_policy::reference::REFERENCE_ID,
            crate::code_policy::reference::REFERENCE_VERSION,
        ));
    }
    for deck in &roster.candidate_deck_pool {
        archetype_of(&decks, deck)?;
    }

    let (opponents, seeds, declared_cells, manifest_sha256) = match split {
        Split::Train => (
            resolve_opponents(&roster.train.opponents, &decks)?,
            roster.train.seeds,
            roster.train.cell_count,
            None,
        ),
        Split::Heldout => {
            let manifest_path = sealed_manifest_path(roster_path, &roster.heldout.manifest);
            let raw = std::fs::read_to_string(&manifest_path).map_err(|error| {
                format!(
                    "sealed heldout manifest missing at {}: {error}. Run \
                     varieties/magic/scripts/build_sealed_heldout.py first; the sealed tree is \
                     gitignored by design and a fresh clone does not have it.",
                    manifest_path.display()
                )
            })?;
            let value: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|error| format!("cannot parse sealed manifest: {error}"))?;
            let digest = sha256::hex_digest(canonical_json(&value)?.as_bytes());
            if digest != roster.heldout.sha256 {
                return Err(format!(
                    "sealed heldout manifest sha256 {digest} does not match the committed roster \
                     pin {}; the heldout surface moved and no score from it is comparable",
                    roster.heldout.sha256
                ));
            }
            let manifest: HeldoutManifest = serde_json::from_value(value)
                .map_err(|error| format!("sealed manifest has the wrong shape: {error}"))?;
            if manifest.schema_version != HELDOUT_SCHEMA {
                return Err(format!(
                    "sealed manifest schema `{}` is not `{HELDOUT_SCHEMA}`",
                    manifest.schema_version
                ));
            }
            if manifest.split != "heldout" {
                return Err(format!(
                    "sealed manifest declares split `{}`, not `heldout`",
                    manifest.split
                ));
            }
            if manifest.opponents.len() != roster.heldout.opponent_count {
                return Err(format!(
                    "sealed manifest has {} opponents, roster publishes {}",
                    manifest.opponents.len(),
                    roster.heldout.opponent_count
                ));
            }
            if manifest.seeds != roster.heldout.seeds {
                return Err(format!(
                    "sealed manifest seeds {} != roster seeds {}",
                    manifest.seeds, roster.heldout.seeds
                ));
            }
            if manifest.cell_count != roster.heldout.cell_count {
                return Err(format!(
                    "sealed manifest publishes {} cells, roster pins {}",
                    manifest.cell_count, roster.heldout.cell_count
                ));
            }
            (
                resolve_opponents(&manifest.opponents, &decks)?,
                manifest.seeds,
                manifest.cell_count,
                Some(digest),
            )
        }
    };

    let surface = Surface {
        split,
        roster_id: roster.roster_id,
        task_id: roster.task_id,
        score_metric: roster.score_metric,
        manifest_sha256,
        reference,
        opponents,
        seeds,
        candidate_deck_pool: roster.candidate_deck_pool,
    };

    // The published cell count is part of the pin. A surface that quietly grew
    // or shrank is a different benchmark wearing the same roster id.
    if surface.cell_count() != declared_cells {
        return Err(format!(
            "{} surface enumerates {} cells but the manifest publishes {declared_cells}",
            split.id(),
            surface.cell_count()
        ));
    }
    if surface.seeds == 0 {
        return Err("a surface with zero seeds measures nothing".to_owned());
    }
    Ok(surface)
}

/// Resolves the sealed manifest path relative to the variety root.
///
/// The manifest reference in the roster is written relative to
/// `varieties/magic/`, which is the roster file's parent's parent.
fn sealed_manifest_path(roster_path: &Path, relative: &str) -> PathBuf {
    let root = roster_path
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    root.join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(seat: Seat, seed: u64) -> Cell {
        Cell {
            candidate_deck: "deck_a".to_owned(),
            candidate_pilot: "v8".to_owned(),
            opponent_id: "opp_t1".to_owned(),
            opponent_deck: "deck_b".to_owned(),
            seat,
            seed,
        }
    }

    /// The two identifiers must differ in exactly the way the pairing needs:
    /// the full id separates arms, the pair key joins them.
    #[test]
    fn the_pair_key_drops_the_candidate_half_and_the_cell_id_keeps_it() {
        let candidate = cell(Seat::Play, 3);
        let mut reference = candidate.clone();
        reference.candidate_deck = "deck_ref".to_owned();
        reference.candidate_pilot = "v5".to_owned();

        assert_ne!(
            candidate.cell_id(),
            reference.cell_id(),
            "the two arms must not collide in the coverage check"
        );
        assert_eq!(
            candidate.pair_key(),
            reference.pair_key(),
            "the two arms must join on the shared coordinate, or nothing pairs"
        );
        assert_eq!(candidate.pair_key(), "opp_t1|deck_b|p0|3");
    }

    /// Seat is a real axis, not decoration: the same seed on the other seat is
    /// a different cell.
    #[test]
    fn seat_and_seed_are_independent_axes() {
        assert_ne!(cell(Seat::Play, 1).pair_key(), cell(Seat::Draw, 1).pair_key());
        assert_ne!(cell(Seat::Play, 1).pair_key(), cell(Seat::Play, 2).pair_key());
        assert_eq!(Seat::Play.index(), 0);
        assert_eq!(Seat::Draw.index(), 1);
    }

    /// The committed roster must resolve, or nothing downstream can run. This
    /// is the guard that catches a roster naming a deck that was renamed away.
    #[test]
    fn the_committed_train_surface_resolves() {
        let surface = load_surface(&committed_roster(), Split::Train)
            .expect("the committed roster must resolve its train split");
        assert_eq!(surface.opponents.len(), 5);
        assert_eq!(surface.cell_count(), surface.cells(&surface.reference).len());
        assert_eq!(
            surface.reference.pilot,
            crate::code_policy::reference::REFERENCE_VERSION
        );
        let ids: BTreeSet<String> = surface
            .cells(&surface.reference)
            .iter()
            .map(Cell::cell_id)
            .collect();
        assert_eq!(
            ids.len(),
            surface.cell_count(),
            "cell ids must be unique or coverage cannot be checked"
        );
    }

    /// Every deck a candidate may submit has to be flyable by an archetype
    /// pilot, or the pool advertises a deck that cannot be entered.
    #[test]
    fn every_candidate_deck_in_the_pool_is_pilotable() {
        let surface = load_surface(&committed_roster(), Split::Train).expect("roster resolves");
        assert!(!surface.candidate_deck_pool.is_empty());
        let decks = playable_fixtures().expect("fixtures load");
        for deck in &surface.candidate_deck_pool {
            archetype_of(&decks, deck).expect("pool deck must declare a known archetype");
        }
    }

    fn committed_roster() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("policies crate has a parent")
            .join("rosters")
            .join("code_policy_v1.json")
    }
}
