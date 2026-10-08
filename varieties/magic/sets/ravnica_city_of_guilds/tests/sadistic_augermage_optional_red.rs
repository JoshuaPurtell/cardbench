//! Source-audit regression: Sadistic Augermage has a mandatory self-dies trigger.

use cardbench_magic_engine::{Effect, TriggerCondition};
use cardbench_magic_rav::{card_definitions, rav_triggered_ability_bindings};

#[test]
fn sadistic_augermage_binding_matches_its_source_death_instruction() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SADISTIC-AUGERMAGE")
        .expect("Sadistic Augermage is catalogued");
    assert_eq!(definition.name, "Sadistic Augermage");
    assert_eq!(definition.power, Some(3));
    assert_eq!(definition.toughness, Some(1));
    assert!(
        definition
            .supported_rules
            .contains(&"source-dies-each-player-puts-card-on-library-top")
    );

    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("the Augermage has a source-death binding");
    assert_eq!(binding.ability.condition, TriggerCondition::Dies);
    assert!(!binding.ability.optional);
    assert!(matches!(
        binding.ability.effects.as_slice(),
        [Effect::PutOneHandCardOnLibraryTopEachPlayer]
    ));
}
