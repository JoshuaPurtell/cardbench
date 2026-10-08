//! Candidate-owned RAV Hunted Lammasu definition and ETB token ability.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement, TokenSpec,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HUNTED-LAMMASU",
        name: "Hunted Lammasu",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "etb-targeted-opponent-horror-token",
        ],
        power: Some(5),
        toughness: Some(5),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HUNTED-LAMMASU",
        ability: TriggeredAbility {
            id: "etb-opponent-horror",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Opponent],
            effects: vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::horror(),
                count: 1,
            }],
        },
    }
}
