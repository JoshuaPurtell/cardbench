//! Candidate-owned RAV Tidewater Minion identity and activated abilities.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost, TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TIDEWATER-MINION",
        name: "Tidewater Minion",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "tap-untap-target-permanent-and-four-lose-defender",
        ],
        power: Some(4),
        toughness: Some(4),
        keywords: vec![Keyword::Defender],
        effects: vec![],
    }
}

/// `{T}: Untap target permanent.`
#[must_use]
pub(crate) fn untap_target_permanent_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-TIDEWATER-MINION",
        ability: ActivatedAbility {
            id: "tap-untap-target-permanent",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Permanent],
            effects: vec![Effect::UntapTargetPermanent],
        },
    }
}

/// `{4}: Tidewater Minion loses defender until end of turn.`
#[must_use]
pub(crate) fn lose_defender_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-TIDEWATER-MINION",
        ability: ActivatedAbility {
            id: "lose-defender-until-end-of-turn",
            mana_cost: ManaCost::new(4),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::RemoveSourceKeywordUntilEndOfTurn {
                keyword: Keyword::Defender,
            }],
        },
    }
}
