//! Red discovery contract for two source-only White RAV spell paths.
//!
//! This keeps the source-branch reconciliation honest: the cards must first
//! exist with their typed targets before their stack effects can be claimed.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn seed_spark_requires_typed_artifact_or_enchantment_destruction_and_tokens() {
    let seed_spark = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SEED-SPARK")
        .expect("Seed Spark definition exists");
    assert_eq!(seed_spark.mana_cost, ManaCost::with_colors(3, [Color::White]));
    assert_eq!(seed_spark.card_types, [CardType::Instant].into());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&seed_spark.id));
    assert!(seed_spark
        .supported_rules
        .contains(&"typed-artifact-or-enchantment-target"));
    assert!(seed_spark.supported_rules.contains(&"saproling-token-creation"));
}

#[test]
fn leave_no_trace_requires_a_typed_radiance_enchantment_destruction_path() {
    let leave_no_trace = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LEAVE-NO-TRACE")
        .expect("Leave No Trace definition exists");
    assert_eq!(
        leave_no_trace.mana_cost,
        ManaCost::with_colors(1, [Color::White])
    );
    assert_eq!(leave_no_trace.card_types, [CardType::Instant].into());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&leave_no_trace.id));
    assert!(leave_no_trace
        .supported_rules
        .contains(&"radiance-enchantment-destruction"));
}
