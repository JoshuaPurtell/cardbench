//! Red promotion contract for Vinelasher Kudzu's land-entry trigger.
//!
//! The existing direct engine regression proves the controller-only trigger,
//! stack window, counter receipt, and opponent-land exclusion. This contract
//! catches the remaining public-fixture and full-manifest gap.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn vinelasher_kudzu_requires_a_shown_landfall_trace_for_full_fidelity() {
    let kudzu = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VINELASHER-KUDZU")
        .expect("Vinelasher Kudzu definition exists");
    assert!(
        kudzu
            .supported_rules
            .contains(&"controlled-land-entry-plus-one-counter"),
        "the live controller-only land-entry trigger must be part of the definition contract"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&kudzu.id),
        "the existing direct trigger regressions plus a shown policy trace make the card complete"
    );

    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|trace| trace.id == "rav_vinelasher_kudzu_landfall_counter")
        .expect("Vinelasher Kudzu shown landfall scenario exists");
    println!("Vinelasher Kudzu red trace: {:#?}", trace.event_log);
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("TriggeredAbilityStacked"))
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("CounterPlaced"))
    );
}
