//! Red discovery contract for Nullstone Gargoyle's first-spell trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Effect, Keyword, ManaCost, TargetRequirement, TriggerCondition,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

#[test]
fn nullstone_gargoyle_requires_first_noncreature_spell_each_turn_counter_trigger() {
    let gargoyle = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NULLSTONE-GARGOYLE")
        .expect("Nullstone Gargoyle definition exists");

    assert_eq!(gargoyle.name, "Nullstone Gargoyle");
    assert_eq!(gargoyle.mana_cost, ManaCost::new(5));
    assert!(gargoyle.colors.is_empty());
    assert_eq!(gargoyle.card_types, BTreeSet::from([CardType::Artifact, CardType::Creature]));
    assert_eq!(gargoyle.power, Some(4));
    assert_eq!(gargoyle.toughness, Some(5));
    assert_eq!(gargoyle.keywords, [Keyword::Flying]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&gargoyle.id));
    assert!(
        gargoyle
            .supported_rules
            .contains(&"first-noncreature-spell-each-player-each-turn-counter")
    );

    let trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-NULLSTONE-GARGOYLE")
        .expect("Nullstone Gargoyle trigger binding exists");
    assert_eq!(
        trigger.ability.condition,
        TriggerCondition::FirstNoncreatureSpellCastEachTurn
    );
    assert_eq!(trigger.ability.targets, [TargetRequirement::NoncreatureSpell]);
    assert_eq!(trigger.ability.effects, [Effect::CounterTargetSpell]);
}
