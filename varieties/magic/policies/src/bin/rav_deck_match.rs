use std::process::ExitCode;

use cardbench_magic_policies::{DeckMatchConfig, run_rav_full_deck_match};

fn main() -> ExitCode {
    match run_rav_full_deck_match(DeckMatchConfig::default()) {
        Ok(result) => {
            println!("schema_version=cardbench.magic.full-deck-policy-match.v1");
            println!("match_id={}", result.id);
            println!("termination={:?}", result.termination);
            println!(
                "winner={}",
                result
                    .winner
                    .map_or("none".to_owned(), |player| player.0.to_string())
            );
            println!("turns={}", result.turns);
            println!("attempted_policy_moves={}", result.attempted_policy_moves);
            println!("accepted_policy_moves={}", result.accepted_policy_moves);
            println!("life_p0={}", result.life[0]);
            println!("life_p1={}", result.life[1]);
            println!("event_digest={}", result.digest);
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
            for event in &result.event_log {
                println!("event={event}");
            }
            if result.is_clean_completion() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("full-deck policy match setup failed: {error}");
            ExitCode::FAILURE
        }
    }
}
