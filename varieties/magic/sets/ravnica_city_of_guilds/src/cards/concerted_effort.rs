//! Candidate-owned RAV Concerted Effort definition and upkeep trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, SharedKeywordFamily, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed identity and bounded shared-keyword rules contract.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CONCERTED-EFFORT",
        name: "Concerted Effort",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "upkeep-controller-creature-keyword-sharing",
            "exact-protection-and-landwalk-instances",
            "other-creature-only-snapshot",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// Snapshots controller-owned creature keywords from the six printed families.
#[must_use]
pub(crate) fn upkeep_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-CONCERTED-EFFORT",
        ability: TriggeredAbility {
            id: "upkeep-share-controller-creature-keywords",
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::ShareControllerCreatureKeywordsUntilEndOfTurn {
                families: vec![
                    SharedKeywordFamily::Flying,
                    SharedKeywordFamily::FirstStrike,
                    SharedKeywordFamily::DoubleStrike,
                    SharedKeywordFamily::Landwalk,
                    SharedKeywordFamily::Protection,
                    SharedKeywordFamily::Trample,
                ],
            }],
        },
    }
}
