//! Candidate-owned RAV Hunted Phantasm definition and Goblin ETB trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement, TokenSpec,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed blue 4/6 creature that cannot be blocked.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HUNTED-PHANTASM",
        name: "Hunted Phantasm",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "cannot-be-blocked",
            "enter-battlefield-targeted-opponent-goblin-token-creation",
            "policy-submitted-trigger-target",
        ],
        power: Some(4),
        toughness: Some(6),
        keywords: vec![Keyword::Unblockable],
        effects: vec![],
    }
}

/// A target opponent creates five red 1/1 Goblin creature tokens.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HUNTED-PHANTASM",
        ability: TriggeredAbility {
            id: "etb-opponent-goblins",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Opponent],
            effects: vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::red_goblin(),
                count: 5,
            }],
        },
    }
}
