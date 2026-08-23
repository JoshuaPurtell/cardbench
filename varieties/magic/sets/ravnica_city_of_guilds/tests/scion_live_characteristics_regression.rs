use cardbench_magic_rav::run_public_scenario;

/// Scion's layer-seven live creature-count effect must not recursively derive
/// its own characteristics while counting the controller's creatures.
#[test]
fn scion_live_characteristics_scenario_runs_without_recursive_derivation() {
    run_public_scenario("rav_scion_of_the_wild_live_characteristics")
        .expect("Scion public scenario should complete without characteristic recursion");
}
