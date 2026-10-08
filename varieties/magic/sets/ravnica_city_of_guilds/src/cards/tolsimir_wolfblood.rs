//! Candidate-owned RAV #236 Tolsimir Wolfblood bindings.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color,
    ContinuousChange, Effect, ManaCost, StaticContinuousEffectBinding, TokenSpec,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TOLSIMIR-WOLFBLOOD",
        name: "Tolsimir Wolfblood",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Green, Color::White]),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "other-green-and-white-creatures-get-plus-one-plus-one",
            "tap-create-named-legendary-green-white-wolf-token",
        ],
        power: Some(3),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-TOLSIMIR-WOLFBLOOD",
        ability: ActivatedAbility {
            id: "tap-create-voja",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::voja(),
                count: 1,
            }],
        },
    }
}

#[must_use]
pub(crate) fn static_continuous_effects() -> [StaticContinuousEffectBinding; 2] {
    [
        StaticContinuousEffectBinding {
            card_definition: "RAV-TOLSIMIR-WOLFBLOOD",
            change: ContinuousChange::OtherControlledCreaturesOfColorModifyPowerToughness {
                color: Color::Green,
                power: 1,
                toughness: 1,
            },
        },
        StaticContinuousEffectBinding {
            card_definition: "RAV-TOLSIMIR-WOLFBLOOD",
            change: ContinuousChange::OtherControlledCreaturesOfColorModifyPowerToughness {
                color: Color::White,
                power: 1,
                toughness: 1,
            },
        },
    ]
}
