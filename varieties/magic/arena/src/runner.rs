//! Running two seats against each other.
//!
//! The pairing is copied from `policies::ladder::run_step` deliberately, not
//! coincidentally: each seed is played twice with the seats swapped, so the
//! play advantage cancels and the reported rate is attributable to the seat
//! rather than to who went first. That crate learned the hard way that pooling
//! a swapped pair also cancels anything seat-dependent *exactly*, so the seat
//! split is reported here too. A model that is strong on the play and weak on
//! the draw must not read as 50%.

use std::sync::{Arc, Mutex};

use cardbench_magic_engine::PlayerId;
use cardbench_magic_policies::{
    Archetype, CodePolicy, DeckMatchConfig, DeckMatchTermination, Interval, PolicyVersion,
    planner::CardIndex, run_deck_matchup_with, seat_policy, shared_card_index,
};

use crate::provider::LlmProvider;
use crate::react::{DecisionRecord, ReactConfig, ReactPolicy, ReactStats};

/// What sits in a seat.
#[derive(Clone)]
pub enum SeatSpec {
    /// A frozen policy generation.
    Code(PolicyVersion),
    /// A model, with a code policy answering whatever it cannot.
    React {
        provider: Arc<dyn LlmProvider>,
        fallback: PolicyVersion,
        config: ReactConfig,
    },
}

impl SeatSpec {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Code(version) => version.id().to_owned(),
            Self::React {
                provider, fallback, ..
            } => format!("react:{}(fallback {})", provider.id(), fallback.id()),
        }
    }

    #[must_use]
    pub const fn is_react(&self) -> bool {
        matches!(self, Self::React { .. })
    }

    fn build(
        &self,
        player: PlayerId,
        archetype: Archetype,
        index: Arc<CardIndex>,
        stats: &Arc<Mutex<ReactStats>>,
        log: &Arc<Mutex<Vec<DecisionRecord>>>,
    ) -> Box<dyn CodePolicy> {
        match self {
            Self::Code(version) => seat_policy(*version, player, archetype, index),
            Self::React {
                provider,
                fallback,
                config,
            } => {
                let inner = seat_policy(*fallback, player, archetype, index.clone());
                Box::new(ReactPolicy::new(
                    player,
                    index,
                    provider.clone(),
                    inner,
                    config.clone(),
                    stats.clone(),
                    log.clone(),
                ))
            }
        }
    }
}

/// One deck's cell.
#[derive(Clone, Debug)]
pub struct DeckOutcome {
    pub deck: String,
    pub archetype: Archetype,
    /// Seat A's win rate, paired across both seats.
    pub win_rate: Interval,
    pub win_rate_on_the_play: Interval,
    pub win_rate_on_the_draw: Interval,
    pub games: u32,
    pub mean_turns: f64,
    /// Proposals the engine refused. Any nonzero value means these games did
    /// not end by play.
    pub rejected_moves: u32,
    /// Games stopped by a turn or move limit rather than a result.
    pub truncated: u32,
}

/// A whole run.
#[derive(Clone, Debug)]
pub struct ArenaResult {
    pub seat_a: String,
    pub seat_b: String,
    pub per_deck: Vec<DeckOutcome>,
    pub overall: Interval,
    pub stats_a: ReactStats,
    pub stats_b: ReactStats,
}

impl ArenaResult {
    /// Whether every cell ended its games by play.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.per_deck.iter().all(|deck| deck.rejected_moves == 0)
    }

    /// Whether seat A is a measured improvement over seat B.
    #[must_use]
    pub fn a_is_stronger(&self) -> bool {
        self.is_valid() && self.overall.point > 0.5 && self.overall.excludes(0.5)
    }
}

/// One game to play: which deck, which seed, and which seat A occupies.
#[derive(Clone, Copy)]
struct Job {
    deck: usize,
    seed: u64,
    a_seat: usize,
}

/// What one game produced.
struct Played {
    turns: u32,
    rejected: u32,
    termination: DeckMatchTermination,
}

/// The two seat specifications, grouped so the worker signature stays legible.
#[derive(Clone, Copy)]
struct Seats<'a> {
    a: &'a SeatSpec,
    b: &'a SeatSpec,
}

/// State every worker shares. The two stats counters are the only things
/// actually written concurrently, and both are already behind a mutex.
#[derive(Clone, Copy)]
struct Shared<'a> {
    index: &'a Arc<CardIndex>,
    stats_a: &'a Arc<Mutex<ReactStats>>,
    stats_b: &'a Arc<Mutex<ReactStats>>,
    log_a: &'a Arc<Mutex<Vec<DecisionRecord>>>,
    log_b: &'a Arc<Mutex<Vec<DecisionRecord>>>,
}

/// Runs every queued game on `jobs` threads and returns the results in job
/// order.
///
/// Order is restored by writing into a slot indexed by job number rather than
/// pushing as games finish, so a run's reported numbers never depend on which
/// thread won a race.
fn play_all(
    queue: &[Job],
    decks: &[(String, Archetype)],
    seats: Seats<'_>,
    shared: Shared<'_>,
    config: &DeckMatchConfig,
    jobs: usize,
    progress: &(impl Fn(&str) + Sync),
) -> Vec<Option<Result<Played, String>>> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = jobs.max(1).min(queue.len().max(1));
    let slots: Vec<Mutex<Option<Result<Played, String>>>> =
        (0..queue.len()).map(|_| Mutex::new(None)).collect();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let position = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(job) = queue.get(position) else {
                        break;
                    };
                    let outcome = play_one(job, decks, seats, shared, config, progress);
                    if let Ok(mut slot) = slots[position].lock() {
                        *slot = Some(outcome);
                    }
                }
            });
        }
    });

    slots
        .into_iter()
        .map(|slot| slot.into_inner().unwrap_or(None))
        .collect()
}

fn play_one(
    job: &Job,
    decks: &[(String, Archetype)],
    seats: Seats<'_>,
    shared: Shared<'_>,
    config: &DeckMatchConfig,
    progress: &(impl Fn(&str) + Sync),
) -> Result<Played, String> {
    let (deck, archetype) = &decks[job.deck];
    let build = |spec: &SeatSpec, player: PlayerId, first: bool| {
        if first {
            spec.build(
                player,
                *archetype,
                shared.index.clone(),
                shared.stats_a,
                shared.log_a,
            )
        } else {
            spec.build(
                player,
                *archetype,
                shared.index.clone(),
                shared.stats_b,
                shared.log_b,
            )
        }
    };
    let pilots: [Box<dyn CodePolicy>; 2] = if job.a_seat == 0 {
        [
            build(seats.a, PlayerId(0), true),
            build(seats.b, PlayerId(1), false),
        ]
    } else {
        [
            build(seats.b, PlayerId(0), false),
            build(seats.a, PlayerId(1), true),
        ]
    };
    let result = run_deck_matchup_with(
        DeckMatchConfig {
            shuffle_seed: job.seed,
            ..*config
        },
        deck,
        deck,
        pilots,
    )?;
    progress(&format!(
        "deck={deck} seed={} a_seat={} turns={} termination={:?}",
        job.seed, job.a_seat, result.turns, result.termination
    ));
    Ok(Played {
        turns: result.turns,
        rejected: result
            .attempted_policy_moves
            .saturating_sub(result.accepted_policy_moves),
        termination: result.termination,
    })
}

/// Folds one deck's games into a cell.
///
/// Reads the results in job order rather than completion order, which is what
/// keeps a parallel run's numbers identical to a sequential one's.
fn tally(
    deck: &str,
    archetype: Archetype,
    deck_index: usize,
    queue: &[Job],
    played: &[Option<Result<Played, String>>],
) -> Result<DeckOutcome, String> {
    let mut wins = 0;
    let mut decided = 0;
    // Indexed by the seat A occupied: 0 is on the play.
    let mut seat_wins = [0_u32; 2];
    let mut seat_decided = [0_u32; 2];
    let mut rejected = 0;
    let mut truncated = 0;
    let mut turns = 0_u64;
    let mut games = 0;

    for (job, outcome) in queue.iter().zip(played.iter()) {
        if job.deck != deck_index {
            continue;
        }
        let result = match outcome {
            Some(Ok(result)) => result,
            Some(Err(error)) => return Err(error.clone()),
            // A scheduled game with no result means a worker died. Reporting a
            // rate over the survivors would silently drop games from the
            // denominator.
            None => return Err(format!("deck {deck}: a scheduled game produced no result")),
        };
        games += 1;
        turns += u64::from(result.turns);
        rejected += result.rejected;
        match result.termination {
            DeckMatchTermination::Winner(PlayerId(seat)) => {
                decided += 1;
                let a_won = u32::from(seat == job.a_seat);
                wins += a_won;
                seat_decided[job.a_seat] += 1;
                seat_wins[job.a_seat] += a_won;
            }
            DeckMatchTermination::Draw => {}
            _ => truncated += 1,
        }
    }

    Ok(DeckOutcome {
        deck: deck.to_owned(),
        archetype,
        win_rate: Interval::wilson(wins, decided),
        win_rate_on_the_play: Interval::wilson(seat_wins[0], seat_decided[0]),
        win_rate_on_the_draw: Interval::wilson(seat_wins[1], seat_decided[1]),
        games,
        mean_turns: if games == 0 {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)] // Turn counts are small.
            {
                turns as f64 / f64::from(games)
            }
        },
        rejected_moves: rejected,
        truncated,
    })
}

/// Plays `seat_a` against `seat_b` over every supplied deck.
///
/// Both seats always play the same deck list, so the only asymmetry is the
/// pilot. That is the same discipline the ladder uses and it is what makes a
/// model-versus-policy number mean anything.
///
/// Games run on `jobs` threads. Every game is independent -- its own engine,
/// its own freshly built pilots, its own seed -- so the only shared state is
/// the two stats counters, which are already behind mutexes. Results are
/// written into a slot indexed by job number and aggregated afterwards in job
/// order, so the reported numbers do not depend on which thread finished first.
///
/// This matters far more for a model seat than for a code one: a game is
/// dominated by round-trip latency, so a sequential run of 24 games against a
/// reasoning model takes hours while the CPU idles.
///
/// # Errors
///
/// Propagates any setup failure from the match runner.
pub fn run(
    seat_a: &SeatSpec,
    seat_b: &SeatSpec,
    decks: &[(String, Archetype)],
    pairs: u32,
    config: &DeckMatchConfig,
    jobs: usize,
    progress: impl Fn(&str) + Sync,
) -> Result<ArenaResult, String> {
    let index = shared_card_index();
    let stats_a = Arc::new(Mutex::new(ReactStats::default()));
    let stats_b = Arc::new(Mutex::new(ReactStats::default()));
    let log_a = Arc::new(Mutex::new(Vec::new()));
    let log_b = Arc::new(Mutex::new(Vec::new()));

    let queue: Vec<Job> = (0..decks.len())
        .flat_map(|deck| {
            (0..u64::from(pairs)).flat_map(move |seed| {
                (0_usize..2).map(move |a_seat| Job {
                    deck,
                    seed,
                    a_seat,
                })
            })
        })
        .collect();

    let played = play_all(
        &queue,
        decks,
        Seats {
            a: seat_a,
            b: seat_b,
        },
        Shared {
            index: &index,
            stats_a: &stats_a,
            stats_b: &stats_b,
            log_a: &log_a,
            log_b: &log_b,
        },
        config,
        jobs,
        &progress,
    );

    let mut per_deck = Vec::new();
    let mut pooled_wins = 0;
    let mut pooled_games = 0;

    for (deck_index, (deck, archetype)) in decks.iter().enumerate() {
        let outcome = tally(deck, *archetype, deck_index, &queue, &played)?;
        pooled_wins += outcome.win_rate.wins;
        pooled_games += outcome.win_rate.samples;
        per_deck.push(outcome);
    }

    let read = |stats: &Arc<Mutex<ReactStats>>| {
        stats
            .lock()
            .map_or_else(|_| ReactStats::default(), |value| *value)
    };
    Ok(ArenaResult {
        seat_a: seat_a.label(),
        seat_b: seat_b.label(),
        per_deck,
        overall: Interval::wilson(pooled_wins, pooled_games),
        stats_a: read(&stats_a),
        stats_b: read(&stats_b),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_contaminated_cell_blocks_a_strength_claim() {
        // The rate here is overwhelming, and the games did not end by play, so
        // it is not evidence of anything about the seat.
        let result = ArenaResult {
            seat_a: "a".to_owned(),
            seat_b: "b".to_owned(),
            per_deck: vec![DeckOutcome {
                deck: "d".to_owned(),
                archetype: Archetype::Aggro,
                win_rate: Interval::wilson(90, 100),
                win_rate_on_the_play: Interval::wilson(45, 50),
                win_rate_on_the_draw: Interval::wilson(45, 50),
                games: 100,
                mean_turns: 12.0,
                rejected_moves: 3,
                truncated: 0,
            }],
            overall: Interval::wilson(90, 100),
            stats_a: ReactStats::default(),
            stats_b: ReactStats::default(),
        };
        assert!(!result.is_valid());
        assert!(!result.a_is_stronger());
    }

    #[test]
    fn a_clean_decisive_result_is_a_strength_claim() {
        let result = ArenaResult {
            seat_a: "a".to_owned(),
            seat_b: "b".to_owned(),
            per_deck: vec![DeckOutcome {
                deck: "d".to_owned(),
                archetype: Archetype::Aggro,
                win_rate: Interval::wilson(70, 100),
                win_rate_on_the_play: Interval::wilson(35, 50),
                win_rate_on_the_draw: Interval::wilson(35, 50),
                games: 100,
                mean_turns: 12.0,
                rejected_moves: 0,
                truncated: 0,
            }],
            overall: Interval::wilson(70, 100),
            stats_a: ReactStats::default(),
            stats_b: ReactStats::default(),
        };
        assert!(result.is_valid());
        assert!(result.a_is_stronger());
    }
}
