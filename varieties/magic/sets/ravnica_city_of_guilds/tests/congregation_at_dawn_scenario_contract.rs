//! Focused public-scenario contract for Congregation at Dawn.

use cardbench_magic_rav::run_public_scenario;

#[test]
fn congregation_at_dawn_ordered_search_scenario_is_independently_executable() {
    let result = run_public_scenario("rav_congregation_at_dawn_ordered_creature_search")
        .expect("Congregation at Dawn's shown scenario executes independently");
    assert_eq!(
        result.id,
        "rav_congregation_at_dawn_ordered_creature_search"
    );
    assert!(
        result
            .event_log
            .iter()
            .any(|event| event.contains("LibrarySearchTopCardsPlaced"))
    );
}
