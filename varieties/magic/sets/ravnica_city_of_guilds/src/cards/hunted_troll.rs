//! Candidate-owned RAV Hunted Troll definition and ability bindings.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement, TokenSpec, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HUNTED-TROLL",
        name: "Hunted Troll",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "etb-targeted-opponent-flying-faerie-tokens",
            "regeneration",
        ],
        power: Some(8),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

/// Hunted Troll's green self-regeneration activation.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-HUNTED-TROLL",
        ability: ActivatedAbility {
            id: "self-regeneration",
            mana_cost: ManaCost::with_colors(0, [Color::Green]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::RegenerateSource],
        },
    }
}

/// The mandatory ETB ability gives the chosen opponent four blue flying Faeries.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HUNTED-TROLL",
        ability: TriggeredAbility {
            id: "etb-opponent-faeries",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Opponent],
            effects: vec![Effect::CreateTokenForTargetOpponent {
                token: TokenSpec::blue_faerie(),
                count: 4,
            }],
        },
    }
}
