//! Red reconciliation contract for the public Necroplasm upkeep scenario.

use std::fs;

use cardbench_magic_rav::{run_all_scenarios, set_root};

#[test]
fn necroplasm_upkeep_scenario_updates_the_shown_corpus_count() {
    let scenarios = run_all_scenarios().expect("public scenarios execute");
    assert!(
        scenarios
            .iter()
            .any(|scenario| scenario.id == "rav_necroplasm_ordered_upkeep_counter_sweep"),
        "the policy-submitted simultaneous-trigger trace remains public"
    );
    let index_path = set_root().join("scenarios/shown.toml");
    let index = fs::read_to_string(&index_path).expect("shown scenario index");
    let declared_count = index
        .lines()
        .find_map(|line| line.trim().strip_prefix("scenario_count = "))
        .expect("shown index declares a scenario count")
        .parse::<usize>()
        .expect("shown count is numeric");
    assert_eq!(
        declared_count,
        scenarios.len(),
        "shown scenario count follows the new Necroplasm trace"
    );
}
