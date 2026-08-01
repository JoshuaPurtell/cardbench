//! Red discovery contract for Chord of Calling's policy-submitted search.

use cardbench_magic_rav::{card_definitions, executable_definition_id_for_collector};

#[test]
fn chord_of_calling_requires_an_executable_policy_submitted_search_definition() {
    let collector_resolution = executable_definition_id_for_collector(156);
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CHORD-OF-CALLING");
    println!(
        "Chord discovery: collector_resolution={collector_resolution:?}, definition_present={}",
        definition.is_some()
    );

    assert_eq!(collector_resolution, Ok("RAV-CHORD-OF-CALLING"));
    assert!(
        definition.is_some(),
        "Chord of Calling must have an executable policy-submitted search definition"
    );
}
