//! Plays the constructed decks against each other and reports the matchup
//! matrix with confidence intervals.
//!
//! Usage: `rav-archetype-matrix [SEED_PAIRS]` (default 50, i.e. 100 games per
//! cell). Every cell plays each seed twice with the seats swapped.

use cardbench_magic_policies::{DeckMatchConfig, Matrix, measure_matrix};
use cardbench_magic_rav::load_constructed_decks;

fn percent(value: f64) -> String {
    format!("{:.1}", value * 100.0)
}

fn main() {
    let pairs: u32 = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(50);

    let decks: Vec<String> = match load_constructed_decks() {
        Ok(decks) => decks.into_iter().map(|deck| deck.id).collect(),
        Err(error) => {
            eprintln!("failed to load constructed decks: {error}");
            std::process::exit(1);
        }
    };

    println!("schema_version=cardbench.magic.archetype-matrix.v1");
    println!("seed_pairs={pairs} games_per_cell={}", pairs * 2);
    println!("deck_count={}", decks.len());

    let matrix = match measure_matrix(&decks, pairs, &DeckMatchConfig::default()) {
        Ok(matrix) => matrix,
        Err(error) => {
            eprintln!("matrix failed: {error}");
            std::process::exit(1);
        }
    };

    report(&matrix);

    // Fail closed on the things that would make every number above a lie.
    let mut failures = 0;
    for finding in matrix.engine_findings() {
        println!(
            "engine_finding kind={:?} code={}",
            finding.kind, finding.code
        );
        failures += 1;
    }
    for mirror in matrix.miscalibrated_mirrors() {
        println!(
            "miscalibrated_mirror deck={} rate={} interval=[{},{}]",
            mirror.deck_a,
            percent(mirror.overall.point),
            percent(mirror.overall.low),
            percent(mirror.overall.high)
        );
        failures += 1;
    }
    let rejected = matrix.rejected_moves();
    if rejected > 0 {
        println!("rejected_policy_moves={rejected}");
        failures += 1;
    }
    println!("failure_count={failures}");
    if failures > 0 {
        std::process::exit(1);
    }
}

fn report(matrix: &Matrix) {
    println!("\n# pairwise (row deck's win rate against column deck, 95% Wilson)");
    for matchup in &matrix.matchups {
        // The play edge carries its interval. Reported bare, it read as a
        // 60-point effect at 60 games and 9 points at 180, and changed sign on
        // three of four mirrors in between.
        let edge = matchup.play_edge();
        println!(
            "matchup a={} b={} overall={}% ci=[{},{}] n={} on_play={}% on_draw={}% play_edge={:+.1}pt ci=[{:+.1},{:+.1}] established={}",
            matchup.deck_a,
            matchup.deck_b,
            percent(matchup.overall.point),
            percent(matchup.overall.low),
            percent(matchup.overall.high),
            matchup.overall.samples,
            percent(matchup.on_the_play.point),
            percent(matchup.on_the_draw.point),
            edge.point * 100.0,
            edge.low * 100.0,
            edge.high * 100.0,
            edge.is_established(),
        );
    }

    println!("\n# diagnostics");
    for matchup in &matrix.matchups {
        let diagnostics = &matchup.diagnostics;
        println!(
            "diag a={} b={} games={} mean_turns={:.1} mean_moves={:.0} winner_life={:.1} loser_life={:.1} truncated={} draws={} rejected={}",
            matchup.deck_a,
            matchup.deck_b,
            diagnostics.games,
            diagnostics.mean_turns,
            diagnostics.mean_accepted_moves,
            diagnostics.mean_winner_life,
            diagnostics.mean_loser_life,
            diagnostics.truncated,
            diagnostics.draws,
            diagnostics.rejected_moves,
        );
    }

    println!("\n# standings (all non-mirror games)");
    for (deck, interval) in matrix.standings() {
        println!(
            "standing deck={} rate={}% ci=[{},{}] n={}",
            deck,
            percent(interval.point),
            percent(interval.low),
            percent(interval.high),
            interval.samples
        );
    }
}
