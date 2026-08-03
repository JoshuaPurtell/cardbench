//! Red discovery contract for Molten Sentry's entry-time shape selection.

use cardbench_magic_rav::{card_definitions, executable_definition_id_for_collector};

#[test]
fn molten_sentry_has_an_executable_entry_coin_flip_definition() {
    assert_eq!(
        executable_definition_id_for_collector(136),
        Ok("RAV-MOLTEN-SENTRY")
    );
    assert!(
        card_definitions()
            .into_iter()
            .any(|definition| definition.id == "RAV-MOLTEN-SENTRY")
    );
}
