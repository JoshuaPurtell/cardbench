use cardbench_magic_policies::run_rav_trigger_probe;

fn main() {
    match run_rav_trigger_probe() {
        Ok(result) => {
            println!("probe_id={}", result.id);
            println!("triggered_ability_count={}", result.triggered_ability_count);
            println!("event_count={}", result.event_log.len());
            println!("event_digest={}", result.digest);
            println!("passed={}", result.passed());
            for (index, event) in result.event_log.iter().enumerate() {
                println!("event[{index}]={event}");
            }
            if !result.passed() {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("trigger probe failed: {error}");
            std::process::exit(1);
        }
    }
}
