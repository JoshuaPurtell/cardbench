use cardbench_magic_rav::run_all_scenarios;

#[test]
fn gaze_delayed_combat_invariant_does_not_break_existing_first_strike_scenario() {
    run_all_scenarios().expect("all public RAV scenarios remain executable");
}
