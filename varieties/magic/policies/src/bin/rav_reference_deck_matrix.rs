use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use cardbench_magic_policies::{EngineTournamentResult, run_rav_reference_deck_matrix};

const DEFAULT_PROBE_SEED_COUNT: u64 = 8;

fn main() -> ExitCode {
    let summary_only = std::env::var_os("RAV_MATRIX_SUMMARY_ONLY").is_some();
    let seed_count = std::env::var("RAV_MATRIX_SEED_COUNT")
        .map_or(Ok(DEFAULT_PROBE_SEED_COUNT), |value| value.parse::<u64>())
        .ok()
        .filter(|count| *count > 0);
    let Some(seed_count) = seed_count else {
        eprintln!("RAV_MATRIX_SEED_COUNT must be a positive integer");
        return ExitCode::FAILURE;
    };
    match run_rav_reference_deck_matrix(0..seed_count) {
        Ok(result) => {
            if let Some(output_root) = std::env::var_os("RAV_MATRIX_OUTPUT_ROOT")
                && let Err(error) = write_event_logs(Path::new(&output_root), &result, seed_count)
            {
                eprintln!("reference deck matrix event-log export failed: {error}");
                return ExitCode::FAILURE;
            }
            println!("schema_version=cardbench.magic.reference-deck-matrix.v1");
            println!("matrix_id={}", result.id);
            println!("match_count={}", result.matches.len());
            println!("failure_count={}", result.failures.len());
            if !summary_only {
                for game in &result.matches {
                    println!(
                        "match=decks:{}-vs-{} seed:{} termination:{:?} winner:{} turns:{} accepted_moves:{} digest:{}",
                        game.deck_ids[0],
                        game.deck_ids[1],
                        game.config.shuffle_seed,
                        game.termination,
                        game.winner
                            .map_or("none".to_owned(), |player| player.0.to_string()),
                        game.turns,
                        game.accepted_policy_moves,
                        game.digest
                    );
                }
            }
            for failure in &result.failures {
                println!("failure={failure:?}");
            }
            if result.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("reference deck matrix setup failed: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Writes one public, canonical event log per full-deck game. The stable
/// filename and TSV manifest preserve all matchup/seed provenance for review
/// without bundling art, card text, or hidden evaluation material.
fn write_event_logs(
    output_root: &Path,
    result: &EngineTournamentResult,
    seed_count: u64,
) -> Result<(), String> {
    let event_log_root = output_root.join("event-logs");
    fs::create_dir_all(&event_log_root)
        .map_err(|error| format!("{}: {error}", event_log_root.display()))?;
    let mut manifest = String::from(
        "deck_p0\tdeck_p1\tseed\ttermination\twinner\tturns\taccepted_moves\tevent_count\tdigest\tlog_file\n",
    );
    for game in &result.matches {
        let filename = format!(
            "{}__vs__{}__seed-{}.log",
            file_component(&game.deck_ids[0]),
            file_component(&game.deck_ids[1]),
            game.config.shuffle_seed,
        );
        let body = format!(
            "schema_version=cardbench.magic.canonical-event-log.v1\nmatrix_id={}\ndeck_p0={}\ndeck_p1={}\nshuffle_seed={}\ntermination={:?}\nwinner={}\nturns={}\nattempted_policy_moves={}\naccepted_policy_moves={}\nevent_count={}\ndigest={}\n\n{}\n",
            result.id,
            game.deck_ids[0],
            game.deck_ids[1],
            game.config.shuffle_seed,
            game.termination,
            game.winner
                .map_or_else(|| "none".to_owned(), |player| player.0.to_string()),
            game.turns,
            game.attempted_policy_moves,
            game.accepted_policy_moves,
            game.event_log.len(),
            game.digest,
            game.event_log.join("\n"),
        );
        fs::write(event_log_root.join(&filename), body)
            .map_err(|error| format!("{}: {error}", event_log_root.join(&filename).display()))?;
        writeln!(
            &mut manifest,
            "{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}",
            game.deck_ids[0],
            game.deck_ids[1],
            game.config.shuffle_seed,
            game.termination,
            game.winner
                .map_or_else(|| "none".to_owned(), |player| player.0.to_string()),
            game.turns,
            game.accepted_policy_moves,
            game.event_log.len(),
            game.digest,
            filename,
        )
        .expect("writing to String cannot fail");
    }
    fs::write(output_root.join("event-log-manifest.tsv"), manifest).map_err(|error| {
        format!(
            "{}: {error}",
            output_root.join("event-log-manifest.tsv").display()
        )
    })?;
    fs::write(
        output_root.join("matrix-summary.txt"),
        format!(
            "schema_version=cardbench.magic.reference-deck-matrix.v1\nmatrix_id={}\nseed_count={seed_count}\nmatch_count={}\nfailure_count={}\n",
            result.id,
            result.matches.len(),
            result.failures.len(),
        ),
    )
    .map_err(|error| format!("{}: {error}", output_root.join("matrix-summary.txt").display()))?;
    Ok(())
}

fn file_component(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}
