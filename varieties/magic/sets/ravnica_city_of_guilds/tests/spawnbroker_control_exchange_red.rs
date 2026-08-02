//! Red regression for a two-target permanent control exchange.
//!
//! This keeps the card boundary explicit: a compatible continuous-control
//! substrate is not enough unless the trigger preserves both target slots,
//! their controller relation, and the power comparison as one exchange.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

const SPAWNBROKER: &str = "RAV-SPAWNBROKER";

#[test]
fn spawnbroker_requires_a_full_two_target_control_exchange() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == SPAWNBROKER)
        .expect("Spawnbroker definition exists");

    assert!(
        definition
            .supported_rules
            .contains(&"etb-optional-two-creature-control-exchange"),
        "Spawnbroker must preserve its optional ETB control-exchange boundary"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"opponent-creature-power-at-most-controlled-target"),
        "the second target needs its first-target power relation"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Spawnbroker must not be promoted without its atomic target-pair exchange"
    );
}
