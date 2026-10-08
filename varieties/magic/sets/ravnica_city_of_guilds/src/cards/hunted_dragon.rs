//! Candidate-owned RAV Hunted Dragon definition and targeted Knight ETB.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement, TokenSpec,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed red 6/6 Dragon with Flying and Haste.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HUNTED-DRAGON",
        name: "Hunted Dragon",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "haste",
            "etb-targeted-opponent-knight-tokens",
        ],
        power: Some(6),
        toughness: Some(6),
        keywords: vec![Keyword::Flying, Keyword::Haste],
        effects: vec![],
    }
}

/// The targeted opponent creates three white 2/2 Knight tokens with First Strike.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HUNTED-DRAGON",
        ability: TriggeredAbility {
            id: "etb-opponent-knights",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Opponent],
            effects: vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::knight(),
                count: 3,
            }],
        },
    }
}
