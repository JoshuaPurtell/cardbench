//! Bounded public contract for Dimir House Guard.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn dimir_house_guard_is_explicit_about_its_supported_and_omitted_rules() {
    let guard = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-HOUSE-GUARD")
        .expect("Dimir House Guard definition exists");
    assert_eq!(
        guard.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "fear",
            "immediate-hand-zone-transmute-compatibility",
        ]
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&guard.id),
        "the sacrifice-to-regenerate activation remains deliberately unclaimed"
    );
}

#[test]
fn dimir_house_guard_public_scenario_records_transmute_and_rejects_a_fear_block() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_dimir_house_guard_fear_and_transmute_compatibility")
        .expect("Dimir House Guard public scenario exists");
    println!(
        "Dimir House Guard compatibility trace: {:?}",
        trace.event_log
    );
    assert_eq!(trace.digest, "fnv1a64:594a2e5e1cc03219");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("Transmuted"))
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("AttackersDeclared"))
    );
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("BlockersDeclared")),
        "an illegal Fear block must not write a blocker-declaration receipt"
    );
}
