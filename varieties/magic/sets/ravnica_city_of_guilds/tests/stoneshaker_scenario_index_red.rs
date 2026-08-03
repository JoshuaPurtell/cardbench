//! Red regression for the Stoneshaker public-scenario index boundary.

use std::fs;

use cardbench_magic_rav::{run_all_scenarios, set_root};

#[test]
fn shown_index_includes_the_stoneshaker_public_scenario() {
    let index_path = set_root().join("scenarios/shown.toml");
    let index = fs::read_to_string(index_path).expect("shown scenario index");
    let declared_count = index
        .lines()
        .find_map(|line| line.trim().strip_prefix("scenario_count = "))
        .expect("scenario index declares scenario_count")
        .parse::<usize>()
        .expect("scenario_count is an integer");
    let executable_count = run_all_scenarios()
        .expect("public RAV scenarios execute")
        .len();

    assert_eq!(
        executable_count, 189,
        "Stoneshaker remains in the shown corpus"
    );
    assert_eq!(
        declared_count, executable_count,
        "shown.toml must track the executable RAV public-scenario corpus"
    );
}
