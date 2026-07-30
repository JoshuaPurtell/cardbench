use std::process::ExitCode;

use cardbench_magic_policies::run_rav_reference_deck_matrix;

const PROBE_SEEDS: std::ops::Range<u64> = 0..8;

fn main() -> ExitCode {
    match run_rav_reference_deck_matrix(PROBE_SEEDS) {
        Ok(result) => {
            println!("schema_version=cardbench.magic.reference-deck-matrix.v1");
            println!("matrix_id={}", result.id);
            println!("match_count={}", result.matches.len());
            println!("failure_count={}", result.failures.len());
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
