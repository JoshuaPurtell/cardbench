use std::process::ExitCode;

fn main() -> ExitCode {
    match cardbench_magic_policies::run_rav_reference_match() {
        Ok(result) => {
            println!("schema_version=cardbench.magic.policy-match.v1");
            println!("match_id={}", result.id);
            println!("passed=true");
            println!("policy_moves={}", result.policy_moves);
            println!("life_p0={}", result.life[0]);
            println!("life_p1={}", result.life[1]);
            println!("token_count={}", result.token_count);
            println!("event_digest={}", result.digest);
            for event in result.event_log {
                println!("event={event}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("policy match verification failed: {error}");
            ExitCode::FAILURE
        }
    }
}
