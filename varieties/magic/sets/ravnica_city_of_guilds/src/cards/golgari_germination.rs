//! Candidate-owned definition and nontoken creature dies trigger for Golgari Germination.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TokenSpec, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOLGARI-GERMINATION",
        name: "Golgari Germination",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Black, Color::Green]),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "controlled-nontoken-creature-dies-create-saproling",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-GOLGARI-GERMINATION",
        ability: TriggeredAbility {
            id: "controlled-nontoken-creature-dies-create-saproling",
            condition: TriggerCondition::ControlledNontokenCreatureDies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 1,
            }],
        },
    }
}
