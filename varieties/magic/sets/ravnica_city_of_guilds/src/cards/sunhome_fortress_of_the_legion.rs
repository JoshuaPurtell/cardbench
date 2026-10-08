//! Candidate-owned RAV Sunhome, Fortress of the Legion identity and abilities.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedManaAbility, CardDefinition, CardType, Color, Effect, Keyword,
    ManaAbilityOutput, ManaCost, TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SUNHOME-FORTRESS",
        name: "Sunhome, Fortress of the Legion",
        set_code: "RAV",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Colorless]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-mana-ability",
            "activated-double-strike-grant",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn colorless_mana_ability() -> ActivatedManaAbility {
    ActivatedManaAbility {
        id: "produce-colorless",
        tap_cost: true,
        output: ManaAbilityOutput::Fixed(Color::Colorless),
        amount: 1,
        life_payment: None,
        controller_damage: None,
    }
}

#[must_use]
pub(crate) fn double_strike_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "grant-target-double-strike",
        mana_cost: ManaCost::with_colors(2, [Color::Red, Color::White]),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Creature],
        effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
            keyword: Keyword::DoubleStrike,
        }],
    }
}
