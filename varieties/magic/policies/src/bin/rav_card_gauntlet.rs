use std::process::ExitCode;

use cardbench_magic_policies::run_rav_catalog_gauntlet;

fn main() -> ExitCode {
    match run_rav_catalog_gauntlet([0]) {
        Ok(result) => {
            println!("schema_version=cardbench.magic.catalog-gauntlet.v1");
            println!("deck_count={}", result.deck_count);
            println!("policy_profile_count={}", result.policy_profile_count);
            println!(
                "covered_definition_count={}",
                result.covered_definition_count
            );
            println!("covered_printing_count={}", result.covered_printing_count);
            println!("match_count={}", result.tournament.matches.len());
            println!("failure_count={}", result.tournament.failures.len());
            for game in &result.tournament.matches {
                println!(
                    "match={} vs {} seed={} termination={:?} turns={} accepted_moves={} digest={}",
                    game.deck_ids[0],
                    game.deck_ids[1],
                    game.config.shuffle_seed,
                    game.termination,
                    game.turns,
                    game.accepted_policy_moves,
                    game.digest
                );
            }
            for failure in &result.tournament.failures {
                println!("failure={failure:?}");
            }
            if result.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("catalog gauntlet setup failed: {error}");
            ExitCode::FAILURE
        }
    }
}
