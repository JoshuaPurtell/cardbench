use std::process::ExitCode;

use cardbench_magic_policies::run_rav_full_deck_sweep;

const DEVELOPMENT_SEEDS: [u64; 4] = [11, 73, 127, 521];

fn main() -> ExitCode {
    match run_rav_full_deck_sweep(DEVELOPMENT_SEEDS) {
        Ok(result) => {
            println!("schema_version=cardbench.magic.full-deck-policy-sweep.v1");
            println!("sweep_id={}", result.id);
            println!("match_count={}", result.matches.len());
            println!("engine_finding_count={}", result.engine_findings.len());
            for game in &result.matches {
                println!(
                    "match=seed:{} termination:{:?} winner:{} turns:{} accepted_moves:{} digest:{}",
                    game.config.shuffle_seed,
                    game.termination,
                    game.winner
                        .map_or("none".to_owned(), |player| player.0.to_string()),
                    game.turns,
                    game.accepted_policy_moves,
                    game.digest
                );
            }
            for finding in &result.engine_findings {
                println!(
                    "engine_finding=kind:{:?} seed:{} player:{} code:{} detail:{}",
                    finding.kind,
                    finding.shuffle_seed,
                    finding
                        .player
                        .map_or("none".to_owned(), |player| player.0.to_string()),
                    finding.code,
                    finding.detail
                );
            }
            if result.engine_findings.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("full-deck policy sweep setup failed: {error}");
            ExitCode::FAILURE
        }
    }
}
