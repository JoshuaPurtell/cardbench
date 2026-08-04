//! Measures each policy generation against the one before it.
//!
//! Usage: `rav-policy-ladder [SEED_PAIRS]` (default 25, i.e. 50 games per deck
//! per rung). Both seats play the same deck list, so the only asymmetry is the
//! pilot.

use cardbench_magic_policies::{Archetype, DeckMatchConfig, LadderStep, run_ladder};
use cardbench_magic_rav::load_constructed_decks;

fn percent(value: f64) -> String {
    format!("{:.1}", value * 100.0)
}

fn main() {
    let pairs: u32 = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(25);

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

    let steps = match run_ladder(&decks, pairs, &DeckMatchConfig::default()) {
        Ok(steps) => steps,
        Err(error) => {
            eprintln!("ladder failed: {error}");
            std::process::exit(1);
        }
    };

    let mut invalid = 0;
    for step in &steps {
        report(step);
        if !step.is_valid() {
            invalid += 1;
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
    for regression in step.regressions() {
        println!(
            "regression deck={} rate={}%",
            regression.deck,
            percent(regression.win_rate.point)
        );
    }
}
