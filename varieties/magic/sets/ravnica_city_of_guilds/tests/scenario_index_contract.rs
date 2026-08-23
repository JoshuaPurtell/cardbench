use std::fs;

use cardbench_magic_rav::{run_all_scenarios, set_root};

#[test]
fn shown_scenario_index_count_matches_the_executable_public_fixture() {
    let index_path = set_root().join("scenarios/shown.toml");
    let index = fs::read_to_string(&index_path).expect("shown scenario index");
    let declared_count = index
        .lines()
        .find_map(|line| line.trim().strip_prefix("scenario_count = "))
        .expect("scenario index declares scenario_count")
        .parse::<usize>()
        .expect("scenario_count is an integer");
    let scenarios = run_all_scenarios().expect("public scenarios execute");

    assert_eq!(
        declared_count,
        scenarios.len(),
        "{} must describe the executable public scenario corpus",
        index_path.display()
    );
}
