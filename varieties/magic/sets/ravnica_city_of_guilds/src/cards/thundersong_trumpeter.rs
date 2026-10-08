//! Candidate-owned definition and combat restriction ability for
//! RAV Thundersong Trumpeter.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost, TargetRequirement,
};

/// Printed identity and creature characteristics.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-THUNDERSONG-TRUMPETER",
        name: "Thundersong Trumpeter",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-prevent-target-combat",
        ],
        power: Some(2),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// Tap activation that temporarily restricts a target creature from combat.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-THUNDERSONG-TRUMPETER",
        ability: ActivatedAbility {
            id: "tap-prevent-target-combat",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::CannotAttackOrBlock,
            }],
        },
    }
}
