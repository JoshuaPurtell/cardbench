//! Candidate-owned RAV Hunted Horror definition and Centaur ETB trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement, TokenSpec,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed black 7/7 creature with Trample.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HUNTED-HORROR",
        name: "Hunted Horror",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "trample",
            "enter-battlefield-targeted-opponent-token-creation",
            "centaur-protection-from-black",
        ],
        power: Some(7),
        toughness: Some(7),
        keywords: vec![Keyword::Trample],
        effects: vec![],
    }
}

/// Gives exactly two green 3/3 Centaurs with Protection from Black to a target opponent.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HUNTED-HORROR",
        ability: TriggeredAbility {
            id: "etb-opponent-centaurs",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Opponent],
            effects: vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::hunted_centaur(),
                count: 2,
            }],
        },
    }
}
