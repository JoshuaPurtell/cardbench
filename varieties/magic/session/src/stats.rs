//! Per-seat match statistics, aggregatable across a campaign.
//!
//! This exists because two policy-improvement leads in a row were inferred
//! from things that do not survive contact with play: a six-game sample said
//! midrange was mana-starved (it was variance, in the opposite direction), and
//! a static card count said activated abilities were the largest remaining gap
//! (they were worth nothing, because the cards carrying them rarely reach the
//! board). Both would have been caught by counting what actually happens
//! across a campaign rather than reading one game or one decklist.
//!
//! Nothing here scores or judges. It counts.

use crate::transcript::MatchTranscript;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What one card is, for the purpose of counting it.
///
/// Populated from the engine at capture time: the event log names objects but
/// never says what they are, so without this no type-based statistic exists.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CardIdentity {
    pub object: u64,
    pub definition: String,
    pub owner: u16,
    pub is_creature: bool,
    pub is_land: bool,
    pub mana_value: u8,
}

/// Object identities for one match, keyed by object.
pub type CardRegistry = BTreeMap<u64, CardIdentity>;

/// One seat's activity across one match.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SeatStats {
    pub seat: u16,
    pub deck: String,
    pub policy: String,
    pub won: bool,
    pub turns: u32,
    /// Cards that reached this seat's hand, by kind.
    pub drew_total: u32,
    pub drew_lands: u32,
    pub drew_creatures: u32,
    /// Spells actually cast, by kind.
    pub cast_total: u32,
    pub cast_creatures: u32,
    pub lands_played: u32,
    /// Mana produced. Compared against spend, this is the clearest single
    /// signal of a planner leaving value unused.
    pub mana_produced: u32,
    pub attacks_declared: u32,
    pub attackers_total: u32,
    pub blocks_declared: u32,
    pub abilities_activated: u32,
    pub damage_dealt_to_opponents: i64,
    pub damage_taken: i64,
    /// Turn the seat first resolved a creature. `None` if it never did.
    pub first_creature_turn: Option<u32>,
}

impl SeatStats {
    /// Creatures cast as a share of creatures drawn. Low means the deck is
    /// drawing threats it cannot deploy.
    #[must_use]
    pub fn creature_conversion(&self) -> f64 {
        if self.drew_creatures == 0 {
            return 0.0;
        }
        f64::from(self.cast_creatures) / f64::from(self.drew_creatures)
    }

    /// Land drops made per turn. Below 1.0 late in a game means either flood
    /// protection or missed drops.
    #[must_use]
    pub fn lands_per_turn(&self) -> f64 {
        if self.turns == 0 {
            return 0.0;
        }
        f64::from(self.lands_played) / f64::from(self.turns)
    }
}

/// Both seats' statistics for one match.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MatchStats {
    pub shuffle_seed: u64,
    pub turns: u32,
    pub termination: String,
    pub seats: Vec<SeatStats>,
}

/// Computes per-seat statistics from a transcript and its card registry.
///
/// Events whose objects are absent from the registry are counted in the
/// totals that do not need identity and skipped in the ones that do, rather
/// than being guessed at. CR 800.4a can remove an object the log still names.
#[must_use]
#[allow(clippy::too_many_lines)] // One flat event table; splitting it hides which kinds are covered.
pub fn summarize(transcript: &MatchTranscript, registry: &CardRegistry) -> MatchStats {
    let manifest = &transcript.manifest;
    let mut seats: Vec<SeatStats> = (0..2_u16)
        .map(|seat| SeatStats {
            seat,
            deck: manifest.decks[usize::from(seat)].clone(),
            policy: manifest.policies[usize::from(seat)].clone(),
            won: manifest.winner == Some(seat),
            turns: manifest.turns,
            ..SeatStats::default()
        })
        .collect();

    // A seat's own turns are the ones where it was the active player.
    let mut own_turns: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];

    for event in &transcript.events {
        let actor = event.facts.seats.first().map(|seat| seat.0);
        let object = event.facts.objects.first().map(|object| object.0);
        let identity = object.and_then(|object| registry.get(&object));

        match event.kind.as_str() {
            "StepBegan" => {
                if let Some(seat) = actor
                    && usize::from(seat) < 2
                {
                    own_turns[usize::from(seat)].insert(event.turn);
                }
            }
            "CardMoved" if event.facts.zone.as_deref() == Some("Hand") => {
                if let Some(identity) = identity
                    && usize::from(identity.owner) < 2
                {
                    let entry = &mut seats[usize::from(identity.owner)];
                    entry.drew_total += 1;
                    entry.drew_lands += u32::from(identity.is_land);
                    entry.drew_creatures += u32::from(identity.is_creature);
                }
            }
            "SpellCast" => {
                if let Some(seat) = actor
                    && usize::from(seat) < 2
                {
                    let is_creature = identity.is_some_and(|identity| identity.is_creature);
                    let entry = &mut seats[usize::from(seat)];
                    entry.cast_total += 1;
                    entry.cast_creatures += u32::from(is_creature);
                    if is_creature && entry.first_creature_turn.is_none() {
                        entry.first_creature_turn = Some(event.turn);
                    }
                }
            }
            "PolicyMoveSubmitted" => {
                if event.facts.move_kind.as_deref() == Some("PlayLand")
                    && let Some(seat) = actor
                    && usize::from(seat) < 2
                {
                    seats[usize::from(seat)].lands_played += 1;
                }
            }
            "ManaAdded" => {
                if let (Some(seat), Some(amount)) = (actor, event.facts.amount)
                    && usize::from(seat) < 2
                {
                    seats[usize::from(seat)].mana_produced +=
                        u32::try_from(amount.max(0)).unwrap_or(0);
                }
            }
            "AttackersDeclared" => {
                if let (Some(seat), Some(count)) = (actor, event.facts.amount)
                    && usize::from(seat) < 2
                    && count > 0
                {
                    let entry = &mut seats[usize::from(seat)];
                    entry.attacks_declared += 1;
                    entry.attackers_total += u32::try_from(count.max(0)).unwrap_or(0);
                }
            }
            "BlockersDeclared" => {
                if let (Some(seat), Some(count)) = (actor, event.facts.amount)
                    && usize::from(seat) < 2
                    && count > 0
                {
                    seats[usize::from(seat)].blocks_declared += 1;
                }
            }
            "AbilityActivated" => {
                if let Some(identity) = identity
                    && usize::from(identity.owner) < 2
                {
                    seats[usize::from(identity.owner)].abilities_activated += 1;
                }
            }
            "DamageDealtToPlayer" => {
                if let (Some(seat), Some(amount)) = (actor, event.facts.amount)
                    && usize::from(seat) < 2
                {
                    seats[usize::from(seat)].damage_taken += amount;
                    let other = 1 - usize::from(seat);
                    seats[other].damage_dealt_to_opponents += amount;
                }
            }
            _ => {}
        }
    }

    for (seat, turns) in own_turns.iter().enumerate() {
        if let Ok(count) = u32::try_from(turns.len()) {
            seats[seat].turns = count;
        }
    }

    MatchStats {
        shuffle_seed: manifest.shuffle_seed,
        turns: manifest.turns,
        termination: manifest.termination.clone(),
        seats,
    }
}

/// Campaign-level aggregate for one deck.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DeckAggregate {
    pub deck: String,
    pub games: u32,
    pub wins: u32,
    pub mean_turns: f64,
    pub mean_drew_creatures: f64,
    pub mean_cast_creatures: f64,
    pub creature_conversion: f64,
    pub mean_lands_played: f64,
    pub mean_mana_produced: f64,
    pub mean_attacks: f64,
    pub mean_blocks: f64,
    pub mean_abilities: f64,
    pub mean_damage_dealt: f64,
    /// Share of games where the seat never declared a single attacker.
    pub never_attacked_rate: f64,
    /// Share of games where the seat never made a single block.
    pub never_blocked_rate: f64,
    /// Mean turn of the first resolved creature; games with none are excluded.
    pub mean_first_creature_turn: f64,
}

/// Folds many matches into one row per deck.
#[must_use]
pub fn aggregate(matches: &[MatchStats]) -> Vec<DeckAggregate> {
    let mut rows: BTreeMap<String, Vec<&SeatStats>> = BTreeMap::new();
    for entry in matches {
        for seat in &entry.seats {
            rows.entry(seat.deck.clone()).or_default().push(seat);
        }
    }
    let mut out: Vec<DeckAggregate> = rows
        .into_iter()
        .map(|(deck, seats)| {
            let n = f64::from(u32::try_from(seats.len()).unwrap_or(1).max(1));
            let mean = |f: &dyn Fn(&SeatStats) -> f64| -> f64 {
                seats.iter().map(|seat| f(seat)).sum::<f64>() / n
            };
            let first_creature: Vec<f64> = seats
                .iter()
                .filter_map(|seat| seat.first_creature_turn.map(f64::from))
                .collect();
            DeckAggregate {
                deck,
                games: u32::try_from(seats.len()).unwrap_or(u32::MAX),
                wins: u32::try_from(seats.iter().filter(|seat| seat.won).count())
                    .unwrap_or(u32::MAX),
                mean_turns: mean(&|seat| f64::from(seat.turns)),
                mean_drew_creatures: mean(&|seat| f64::from(seat.drew_creatures)),
                mean_cast_creatures: mean(&|seat| f64::from(seat.cast_creatures)),
                // Ratio of sums, not mean of ratios: a seat that drew no
                // creatures would otherwise contribute a spurious zero and
                // drag the campaign figure below every individual game.
                creature_conversion: {
                    let drew: u32 = seats.iter().map(|seat| seat.drew_creatures).sum();
                    let cast: u32 = seats.iter().map(|seat| seat.cast_creatures).sum();
                    if drew == 0 {
                        0.0
                    } else {
                        f64::from(cast) / f64::from(drew)
                    }
                },
                mean_lands_played: mean(&|seat| f64::from(seat.lands_played)),
                mean_mana_produced: mean(&|seat| f64::from(seat.mana_produced)),
                mean_attacks: mean(&|seat| f64::from(seat.attacks_declared)),
                mean_blocks: mean(&|seat| f64::from(seat.blocks_declared)),
                mean_abilities: mean(&|seat| f64::from(seat.abilities_activated)),
                mean_damage_dealt: mean(&|seat| {
                    f64::from(i32::try_from(seat.damage_dealt_to_opponents).unwrap_or(i32::MAX))
                }),
                never_attacked_rate: mean(&|seat| f64::from(u8::from(seat.attacks_declared == 0))),
                never_blocked_rate: mean(&|seat| f64::from(u8::from(seat.blocks_declared == 0))),
                mean_first_creature_turn: if first_creature.is_empty() {
                    0.0
                } else {
                    first_creature.iter().sum::<f64>()
                        / f64::from(u32::try_from(first_creature.len()).unwrap_or(1))
                },
            }
        })
        .collect();
    out.sort_by(|left, right| left.deck.cmp(&right.deck));
    out
}
