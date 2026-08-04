//! Measures policy generations against each other.
//!
//! ```text
//! rav-policy-ladder [SEED_PAIRS]                      every successive pair
//! rav-policy-ladder [SEED_PAIRS] CHALLENGER INCUMBENT one named pair
//! rav-policy-ladder [SEED_PAIRS] ... --elo PATH       append policy results
//! ```
//!
//! `SEED_PAIRS` defaults to 25, i.e. 50 games per deck per rung. Both seats
//! play the same deck list, so the only asymmetry is the pilot.
//!
//! The named form exists because "is this new policy better than the one we
//! ship" is a different question from "did each rung beat the one before it".
//! A policy written against this harness -- by a person or by a model -- needs
//! to be scored against a chosen baseline, not only against its immediate
//! predecessor, and successive rungs cannot answer that once a change is split
//! across two generations.

use cardbench_magic_policies::{
    Archetype, DeckMatchConfig, EloLedger, LadderStep, PolicyVersion, run_ladder, run_step,
};
use cardbench_magic_rav::load_constructed_decks;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn percent(value: f64) -> String {
    format!("{:.1}", value * 100.0)
}

/// A signed difference in win-rate points, always with its sign shown so the
/// direction is unambiguous in a log.
fn points(value: f64) -> String {
    format!("{:+.1}", value * 100.0)
}

/// The explicit challenger and incumbent, when both were named.
///
/// Exits rather than falling back to the full ladder when a name is
/// unrecognised: silently measuring something other than what was asked for is
/// how a result gets attributed to the wrong policy.
fn named_pair() -> Option<(PolicyVersion, PolicyVersion)> {
    let arguments: Vec<String> = std::env::args().skip(2).take(2).collect();
    let [challenger, incumbent] = arguments.as_slice() else {
        return None;
    };
    let parse = |name: &str| {
        PolicyVersion::parse(name).unwrap_or_else(|| {
            let known: Vec<&str> = PolicyVersion::ALL.iter().map(|entry| entry.id()).collect();
            eprintln!(
                "unknown policy version `{name}`; known: {}",
                known.join(" ")
            );
            std::process::exit(2);
        })
    };
    Some((parse(challenger), parse(incumbent)))
}

fn elo_path() -> Result<Option<PathBuf>, String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut path = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument == "--elo" {
            index += 1;
            let value = arguments
                .get(index)
                .ok_or_else(|| "--elo requires a ledger path".to_owned())?;
            path = Some(PathBuf::from(value));
        } else if let Some(value) = argument.strip_prefix("--elo=") {
            if value.is_empty() {
                return Err("--elo requires a ledger path".to_owned());
            }
            path = Some(PathBuf::from(value));
        }
        index += 1;
    }
    Ok(path)
}

#[allow(clippy::too_many_lines)] // Keep CLI setup, measurement, and ledger output together.
fn main() {
    let pairs: u32 = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(25);
    let elo_path = match elo_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let decks: Vec<(String, Archetype)> = match load_constructed_decks() {
        Ok(decks) => decks
            .into_iter()
            .filter_map(|deck| {
                Archetype::parse(&deck.archetype).map(|archetype| (deck.id, archetype))
            })
            .collect(),
        Err(error) => {
            eprintln!("failed to load constructed decks: {error}");
            std::process::exit(1);
        }
    };

    println!("schema_version=cardbench.magic.policy-ladder.v1");
    println!(
        "seed_pairs={pairs} games_per_deck_per_rung={} deck_count={}",
        pairs * 2,
        decks.len()
    );

    let mut elo = match elo_path.as_deref() {
        Some(path) => match EloLedger::load(path) {
            Ok(ledger) => Some(ledger),
            Err(error) => {
                eprintln!("failed to load Elo ledger: {error}");
                std::process::exit(1);
            }
        },
        None => None,
    };

    let named = named_pair();
    let config = DeckMatchConfig::default();
    let steps = match named {
        Some((challenger, incumbent)) => {
            run_step(challenger, incumbent, &decks, pairs, &config).map(|step| vec![step])
        }
        None => run_ladder(&decks, pairs, &config),
    };
    let steps = match steps {
        Ok(steps) => steps,
        Err(error) => {
            eprintln!("ladder failed: {error}");
            std::process::exit(1);
        }
    };

    let mut invalid = 0;
    let mut elo_updates = 0;
    let mut elo_axes = BTreeSet::new();
    for step in &steps {
        report(step);
        if let Some(ledger) = elo.as_mut() {
            for record in &step.elo_records {
                elo_axes.insert(record.axis.clone());
                match ledger.record_match(record.clone()) {
                    Ok(true) => elo_updates += 1,
                    Ok(false) => {}
                    Err(error) => {
                        eprintln!("failed to record policy Elo result: {error}");
                        invalid += 1;
                    }
                }
            }
        }
        if !step.is_valid() {
            invalid += 1;
        }
    }

    if let (Some(path), Some(ledger)) = (elo_path.as_deref(), elo.as_ref()) {
        if let Err(error) = ledger.save(path) {
            eprintln!("failed to save Elo ledger: {error}");
            invalid += 1;
        } else {
            println!(
                "elo_ledger={} axis=policy added_pairs={} stored_pairs={}",
                path.display(),
                elo_updates,
                ledger.record_count()
            );
            for axis in elo_axes {
                println!("elo_standings axis={axis}");
                for standing in ledger.standings(&axis) {
                    println!(
                        "elo id={} rating={:.1} paired_matches={} anchored={}",
                        standing.id, standing.rating, standing.matches, standing.anchored
                    );
                }
            }
        }
    }
    println!("\ninvalid_rungs={invalid}");
    if invalid > 0 {
        std::process::exit(1);
    }
}

fn report(step: &LadderStep) {
    println!(
        "\n# {} vs {} (challenger win rate; 50% means no change)",
        step.challenger, step.incumbent
    );
    for verdict in &step.per_deck {
        println!(
            "rung challenger={} incumbent={} deck={} archetype={} rate={}% ci=[{},{}] n={} mean_turns={:.1} rejected={} truncated={}",
            step.challenger,
            step.incumbent,
            verdict.deck,
            verdict.archetype,
            percent(verdict.win_rate.point),
            percent(verdict.win_rate.low),
            percent(verdict.win_rate.high),
            verdict.win_rate.samples,
            verdict.mean_turns,
            verdict.rejected_moves,
            verdict.truncated,
        );
        // The paired rate above cancels anything seat-dependent. This is the
        // half it cancels, reported so a 50% rung cannot silently mean "the
        // change moved play but the pairing subtracted it out".
        let asymmetry = verdict.seat_asymmetry();
        println!(
            "  seats deck={} on_play={}% on_draw={}% asymmetry={}pt ci=[{},{}] established={}",
            verdict.deck,
            percent(verdict.win_rate_on_the_play.point),
            percent(verdict.win_rate_on_the_draw.point),
            points(asymmetry.point),
            points(asymmetry.low),
            points(asymmetry.high),
            asymmetry.is_established(),
        );
    }
    println!(
        "overall challenger={} incumbent={} rate={}% ci=[{},{}] n={} improvement={}",
        step.challenger,
        step.incumbent,
        percent(step.overall.point),
        percent(step.overall.low),
        percent(step.overall.high),
        step.overall.samples,
        step.is_improvement(),
    );
    println!(
        "clean challenger={} incumbent={} rate={}% ci=[{},{}] n={} improvement={}",
        step.challenger,
        step.incumbent,
        percent(step.clean.point),
        percent(step.clean.low),
        percent(step.clean.high),
        step.clean.samples,
        step.is_clean_improvement(),
    );
    let asymmetry = step.seat_asymmetry();
    println!(
        "asymmetry challenger={} incumbent={} pooled={}pt ci=[{},{}] established={}",
        step.challenger,
        step.incumbent,
        points(asymmetry.point),
        points(asymmetry.low),
        points(asymmetry.high),
        asymmetry.is_established(),
    );
    for verdict in step.seat_asymmetric() {
        println!(
            "seat-asymmetric deck={} on_play={}% on_draw={}% (paired rate {}% hides this)",
            verdict.deck,
            percent(verdict.win_rate_on_the_play.point),
            percent(verdict.win_rate_on_the_draw.point),
            percent(verdict.win_rate.point),
        );
    }
    for verdict in step.contaminated() {
        println!(
            "contaminated deck={} rejected={} of {} games (see ENGINE_BUG_LEDGER.md)",
            verdict.deck, verdict.rejected_moves, verdict.games
        );
    }
    for regression in step.regressions() {
        println!(
            "regression deck={} rate={}%",
            regression.deck,
            percent(regression.win_rate.point)
        );
    }
}
