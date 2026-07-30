use std::process::ExitCode;
use std::{env, fs, path::PathBuf};

fn main() -> ExitCode {
    if let Err(error) = cardbench_magic_rav::validate_block_manifests() {
        eprintln!("manifest verification failed: {error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = cardbench_magic_rav::validate_shown_deck_pool() {
        eprintln!("deck fixture verification failed: {error}");
        return ExitCode::FAILURE;
    }
    let first = match cardbench_magic_rav::verify_reference_event_logs() {
        Ok(results) => results,
        Err(error) => {
            eprintln!("scenario verification failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    let second = match cardbench_magic_rav::verify_reference_event_logs() {
        Ok(results) => results,
        Err(error) => {
            eprintln!("determinism replay failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if first != second {
        eprintln!("event-log parity failed: same scenarios produced different results");
        return ExitCode::FAILURE;
    }
    println!("schema_version=cardbench.magic.engine-parity.v1");
    println!("task_id=cardbench/magic/engine");
    println!("passed=true");
    println!("scenario_count={}", first.len());
    for result in first {
        println!(
            "scenario={} digest={} summary={}",
            result.id, result.digest, result.summary
        );
    }
    if let Some(output_root) = output_root() {
        if let Err(error) = write_harbor_result(&output_root, &second) {
            eprintln!("could not write engine verifier result: {error}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn output_root() -> Option<PathBuf> {
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn write_harbor_result(
    root: &PathBuf,
    results: &[cardbench_magic_rav::ScenarioResult],
) -> std::io::Result<()> {
    fs::create_dir_all(root)?;
    let scenarios = results
        .iter()
        .map(|result| {
            format!(
                "{{\"id\":\"{}\",\"digest\":\"{}\"}}",
                result.id, result.digest
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let result = format!(
        "{{\n  \"schema_version\": \"cardbench.magic.engine.check.v1\",\n  \"task_id\": \"cardbench/magic/engine\",\n  \"compile_passed\": true,\n  \"event_log_parity_run\": true,\n  \"scenario_count\": {},\n  \"scenarios\": [{}],\n  \"passed\": true,\n  \"harbor_reward\": 1.0\n}}\n",
        results.len(),
        scenarios
    );
    fs::write(root.join("engine-check.json"), result)?;
    fs::write(root.join("reward.txt"), "1.0\n")
}
