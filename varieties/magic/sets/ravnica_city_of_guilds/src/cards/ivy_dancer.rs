//! Candidate-owned definition and targeted Forestwalk ability for Ivy Dancer.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, BasicLandType, CardDefinition, CardType, Color,
    Effect, Keyword, ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-IVY-DANCER",
        name: "Ivy Dancer",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-target-creature-grant-forestwalk",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-IVY-DANCER",
        ability: ActivatedAbility {
            id: "tap-target-creature-grant-forestwalk",
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
                keyword: Keyword::Landwalk(BasicLandType::Forest),
            }],
        },
    }
}
