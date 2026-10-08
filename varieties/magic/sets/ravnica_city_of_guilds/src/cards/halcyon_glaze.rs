//! Candidate-owned RAV Halcyon Glaze definition and creature-spell trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CreatureSubtype, Effect, Keyword, ManaCost, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};

/// RAV #54 is a blue `{1}{U}{U}` enchantment that animates itself after its
/// controller casts a creature spell. Illusion is retained as an animation
/// subtype by the temporary characteristic effect.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HALCYON-GLAZE",
        name: "Halcyon Glaze",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "creature-spell-triggered-self-animation",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// “Whenever you cast a creature spell, Halcyon Glaze becomes a 4/4 Illusion
/// creature with flying in addition to its other types until end of turn.”
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HALCYON-GLAZE",
        ability: TriggeredAbility {
            id: "creature-spell-animate-source-until-end-of-turn",
            condition: TriggerCondition::CastsCreatureSpell,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AnimateSourceIntoCreatureUntilEndOfTurn {
                colors: BTreeSet::from([Color::Blue]),
                creature_subtypes: BTreeSet::from([CreatureSubtype::Illusion]),
                power: 4,
                toughness: 4,
                keywords: vec![Keyword::Flying],
            }],
        },
    }
}
