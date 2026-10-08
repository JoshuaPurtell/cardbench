//! Candidate-owned RAV Ethereal Usher definition and activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost, TargetRequirement,
};

/// RAV #47 is a blue 2/3 Spirit with Transmute {1}{U}{U}.
/// `CardDefinition` has no subtype field, so Spirit remains source identity only.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-ETHEREAL-USHER",
        name: "Ethereal Usher",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-target-unblockable-until-end-of-turn",
            "transmute",
        ],
        power: Some(2),
        toughness: Some(3),
        keywords: vec![Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue],
        ))],
        effects: vec![],
    }
}

/// `{U}, {T}: Target creature can't be blocked this turn.`
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-ETHEREAL-USHER",
        ability: ActivatedAbility {
            id: "tap-target-unblockable",
            mana_cost: ManaCost::with_colors(0, [Color::Blue]),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Unblockable,
            }],
        },
    }
}
