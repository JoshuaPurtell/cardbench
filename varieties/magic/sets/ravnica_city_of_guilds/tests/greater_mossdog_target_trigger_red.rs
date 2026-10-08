//! Negative regression: Greater Mossdog has Dredge 3, but no target trigger.

use cardbench_magic_engine::Keyword;
use cardbench_magic_rav::{card_definitions, rav_triggered_ability_bindings};

#[test]
fn greater_mossdog_has_no_creature_becomes_target_trigger() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GREATER-MOSSDOG")
        .expect("Greater Mossdog definition exists");

    assert_eq!(definition.keywords, [Keyword::Dredge(3)]);
    assert!(definition.effects.is_empty());
    assert!(definition
        .supported_rules
        .iter()
        .all(|rule| *rule != "targeted-creature-return-from-graveyard"));
    assert!(rav_triggered_ability_bindings()
        .iter()
        .all(|binding| binding.card_definition != "RAV-GREATER-MOSSDOG"));
}
