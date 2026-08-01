//! Regression for private-opponent-library choice invariant scoping.

use cardbench_magic_rav::run_all_scenarios;

#[test]
fn unrelated_ability_terminals_do_not_break_private_opponent_choice_auditing() {
    let civic_wayfinder = run_all_scenarios()
        .expect("private-opponent choice invariants allow unrelated ability terminals")
        .into_iter()
        .find(|scenario| scenario.id == "rav_civic_wayfinder_etb_basic_land_search")
        .expect("Civic Wayfinder public scenario exists");
    println!("Civic Wayfinder invariant trace: {:?}", civic_wayfinder.event_log);
}
