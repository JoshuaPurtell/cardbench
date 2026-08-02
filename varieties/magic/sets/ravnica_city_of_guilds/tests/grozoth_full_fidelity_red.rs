//! Red discovery contract for Grozoth's complete ETB search and Transmute.

use cardbench_magic_engine::{
    Effect, LibrarySearchCardinality, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, TriggerCondition,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

#[test]
fn grozoth_requires_its_optional_multi_search_trigger_and_stack_backed_transmute() {
    let grozoth = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GROZOTH")
        .expect("Grozoth definition exists");
    let ability = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == grozoth.id
                && binding.ability.id == "etb-search-mana-value-nine"
        })
        .expect("Grozoth ETB search trigger exists");

    assert_eq!(
        ability.ability.condition,
        TriggerCondition::EntersBattlefield
    );
    assert!(
        ability.ability.optional,
        "the ETB library search is optional"
    );
    assert!(ability.ability.targets.is_empty());
    assert_eq!(
        ability.ability.effects,
        [Effect::SearchControllerLibraryMany {
            requirement: LibrarySearchRequirement::ManaValueExactly(9),
            destination: LibrarySearchDestination::Hand,
            cardinality: LibrarySearchCardinality::ZeroOrMore { maximum: u8::MAX },
            selection: LibrarySearchSelection::PolicySubmitted {
                may_fail_to_find: false,
            },
            reveal_selected: true,
        }]
    );
    assert!(
        grozoth
            .supported_rules
            .contains(&"optional-private-multi-card-mana-value-search")
    );
    assert!(
        grozoth
            .supported_rules
            .contains(&"stack-backed-private-transmute")
    );
    assert!(
        !grozoth
            .supported_rules
            .contains(&"immediate-hand-zone-transmute-compatibility")
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&grozoth.id),
        "the ETB multi-search and stack-backed Transmute complete Grozoth"
    );
}
