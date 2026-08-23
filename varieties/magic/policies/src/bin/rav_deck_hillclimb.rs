//! Scores deck candidates against the frozen constructed-deck controls.
//!
//! Usage: `rav-deck-hillclimb [SEED_PAIRS] [CANDIDATE_ID] [--elo PATH]`
//! (default 10 pairs). Candidates live in the separate RAV hill-climb index, so
//! this command cannot change the deck list used by the policy ladder or the
//! public four-deck matrix.

use cardbench_magic_policies::{
    DEFAULT_RATING, DeckMatchConfig, EloLedger, Interval, Matchup, RatingStanding, measure_matchup,
};
use cardbench_magic_rav::{DeckFixture, load_constructed_decks, load_hillclimb_decks};
use std::path::PathBuf;

struct MatchupScore {
    interval: Interval,
    rejected: u32,
    truncated: u32,
    mean_turns: f64,
    productive_games: u32,
    issues: Vec<String>,
    matchup: Matchup,
}

struct Options {
    pairs: u32,
    requested: Option<String>,
    elo_path: Option<PathBuf>,
}

fn percent(value: f64) -> String {
    format!("{:.1}", value * 100.0)
}

fn usage() -> &'static str {
    "usage: rav-deck-hillclimb [SEED_PAIRS] [CANDIDATE_ID] [--elo PATH]"
}

fn parse_options() -> Result<Options, String> {
    let mut positionals = Vec::new();
    let mut elo_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--help" || argument == "-h" {
            println!("{}", usage());
            println!("  --elo PATH  append clean paired results and print deck standings");
            std::process::exit(0);
        }
        if argument == "--elo" {
            elo_path = Some(PathBuf::from(
                args.next()
                    .ok_or_else(|| "--elo requires a ledger path".to_owned())?,
            ));
        } else if let Some(path) = argument.strip_prefix("--elo=") {
            if path.is_empty() {
                return Err("--elo requires a ledger path".to_owned());
            }
            elo_path = Some(PathBuf::from(path));
        } else if argument.starts_with('-') {
            return Err(format!("unknown option `{argument}`"));
        } else {
            positionals.push(argument);
        }
    }
    if positionals.len() > 2 {
        return Err("too many positional arguments".to_owned());
    }
    let pairs = positionals.first().map_or(Ok(10), |value| {
        value
            .parse::<u32>()
            .map_err(|error| format!("invalid seed-pair count `{value}`: {error}"))
    })?;
    Ok(Options {
        pairs,
        requested: positionals.get(1).cloned(),
        elo_path,
    })
}

fn main() {
    let options = match parse_options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{}\n{}", error, usage());
            std::process::exit(2);
        }
    };
    let pairs = options.pairs;
    let controls = match load_constructed_decks() {
        Ok(decks) => decks,
        Err(error) => {
            eprintln!("failed to load constructed decks: {error}");
            std::process::exit(1);
        }
    };
    let mut candidates = match load_hillclimb_decks() {
        Ok(decks) => decks,
        Err(error) => {
            eprintln!("failed to load hill-climb decks: {error}");
            std::process::exit(1);
        }
    };
    if let Some(requested) = options.requested {
        candidates.retain(|candidate| candidate.id == requested);
        if candidates.is_empty() {
            eprintln!("unknown candidate `{requested}`");
            std::process::exit(2);
        }
    }

    println!("schema_version=cardbench.magic.deck-hillclimb.v1");
    println!(
        "policy_version=latest seed_pairs={pairs} controls={}",
        controls.len()
    );

    let mut elo = match options.elo_path.as_deref() {
        Some(path) => match EloLedger::load(path) {
            Ok(mut ledger) => {
                for control in &controls {
                    if let Err(error) = ledger.ensure_anchor("deck", &control.id, DEFAULT_RATING) {
                        eprintln!("failed to anchor {}: {error}", control.id);
                        std::process::exit(1);
                    }
                }
                Some(ledger)
            }
            Err(error) => {
                eprintln!("failed to load Elo ledger: {error}");
                std::process::exit(1);
            }
        },
        None => None,
    };
    let mut failures = 0;
    let mut elo_updates = 0;
    for candidate in &candidates {
        if let Err(error) = report_candidate(
            candidate,
            &controls,
            pairs,
            &mut failures,
            &mut elo,
            &mut elo_updates,
        ) {
            eprintln!("candidate={} Elo update failed: {error}", candidate.id);
            failures += 1;
        }
    }
    if let (Some(path), Some(ledger)) = (options.elo_path.as_deref(), elo.as_ref()) {
        if let Err(error) = ledger.save(path) {
            eprintln!("failed to save Elo ledger: {error}");
            failures += 1;
        } else {
            println!(
                "elo_ledger={} added_pairs={} stored_pairs={}",
                path.display(),
                elo_updates,
                ledger.record_count()
            );
            print_standings(ledger.standings("deck"));
        }
    }
    println!("failure_count={failures}");
    if failures > 0 {
        std::process::exit(1);
    }
}

fn report_candidate(
    candidate: &DeckFixture,
    controls: &[DeckFixture],
    pairs: u32,
    failures: &mut u32,
    elo: &mut Option<EloLedger>,
    elo_updates: &mut usize,
) -> Result<(), String> {
    let mut wins = 0_u32;
    let mut games = 0_u32;
    let mut productive_total = 0_u32;
    println!(
        "\n# candidate={} archetype={}",
        candidate.id, candidate.archetype
    );
    for control in controls {
        match score(candidate, control, pairs) {
            Ok(score) => {
                let MatchupScore {
                    interval,
                    rejected,
                    truncated,
                    mean_turns,
                    productive_games,
                    issues,
                    matchup,
                } = score;
                if let Some(ledger) = elo.as_mut() {
                    *elo_updates += ledger.record_matchup("deck", &matchup)?;
                }
                println!(
                    "matchup candidate={} control={} rate={}% ci=[{},{}] n={} mean_turns={:.1} productive={}% rejected={} truncated={}",
                    candidate.id,
                    control.id,
                    percent(interval.point),
                    percent(interval.low),
                    percent(interval.high),
                    interval.samples,
                    mean_turns,
                    percent(f64::from(productive_games) / f64::from(interval.samples.max(1))),
                    rejected,
                    truncated,
                );
                for issue in issues {
                    println!(
                        "  issue candidate={} control={} {}",
                        candidate.id, control.id, issue
                    );
                }
                wins += interval.wins;
                games += interval.samples;
                productive_total += productive_games;
                if rejected > 0 || truncated > 0 {
                    *failures += 1;
                }
            }
            Err(error) => {
                eprintln!(
                    "candidate={} control={} failed: {error}",
                    candidate.id, control.id
                );
                *failures += 1;
            }
        }
    }
    let aggregate = Interval::wilson(wins, games);
    println!(
        "aggregate candidate={} rate={}% ci=[{},{}] n={} productive={}% improvement={}",
        candidate.id,
        percent(aggregate.point),
        percent(aggregate.low),
        percent(aggregate.high),
        aggregate.samples,
        percent(f64::from(productive_total) / f64::from(games.max(1))),
        aggregate.low > 0.5,
    );
    Ok(())
}

fn print_standings(standings: Vec<RatingStanding>) {
    println!("elo_standings axis=deck");
    for standing in standings {
        println!(
            "elo id={} rating={:.1} paired_matches={} anchored={}",
            standing.id, standing.rating, standing.matches, standing.anchored
        );
    }
}

fn score(
    candidate: &DeckFixture,
    control: &DeckFixture,
    pairs: u32,
) -> Result<MatchupScore, String> {
    let matchup = measure_matchup(
        &candidate.id,
        &control.id,
        0..u64::from(pairs),
        &DeckMatchConfig::default(),
    )?;
    let issues = matchup
        .games
        .iter()
        .filter(|game| !game.decisive)
        .map(|game| {
            format!(
                "seed={} termination={:?}",
                game.shuffle_seed, game.termination
            )
        })
        .collect();
    let productive_games = matchup
        .games
        .iter()
        .filter(|game| game.decisive && (8..=25).contains(&game.turns))
        .count();
    Ok(MatchupScore {
        interval: matchup.overall,
        rejected: matchup.diagnostics.rejected_moves,
        truncated: matchup.diagnostics.truncated,
        mean_turns: matchup.diagnostics.mean_turns,
        productive_games: u32::try_from(productive_games).unwrap_or(u32::MAX),
        issues,
        matchup,
    })
}
