//! CardBench-authored RAV Boros Guildmage identity and activated abilities.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, HybridManaSymbol, Keyword, ManaCost,
    TargetRequirement,
};

/// Printed characteristics for the red-white hybrid creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BOROS-GUILDMAGE",
        name: "Boros Guildmage",
        set_code: "RAV",
        mana_cost: ManaCost::with_hybrid(
            0,
            [],
            [
                HybridManaSymbol {
                    first: Color::Red,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Red,
                    second: Color::White,
                },
            ],
        ),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-grant-haste",
            "activated-grant-first-strike",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// The red activation grants Haste to a target creature until end of turn.
#[must_use]
pub(crate) fn grant_haste_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "grant-haste",
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Creature],
        effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
            keyword: Keyword::Haste,
        }],
    }
}

/// The white activation grants First Strike to a target creature until end of turn.
#[must_use]
pub(crate) fn grant_first_strike_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "grant-first-strike",
        mana_cost: ManaCost::with_colors(0, [Color::White]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Creature],
        effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
            keyword: Keyword::FirstStrike,
        }],
    }
}
