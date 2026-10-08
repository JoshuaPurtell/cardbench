//! Candidate-owned RAV Loxodon Hierarch identity and abilities.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LOXODON-HIERARCH",
        name: "Loxodon Hierarch",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Green, Color::White]),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "etb-gain-four-life",
            "sacrifice-source-regenerate-controller-creatures",
        ],
        power: Some(4),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

/// When Loxodon Hierarch enters the battlefield, you gain 4 life.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-LOXODON-HIERARCH",
        ability: TriggeredAbility {
            id: "etb-gain-four-life",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 4 }],
        },
    }
}

/// `{G}{W}, Sacrifice Loxodon Hierarch: Regenerate each creature you control.`
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-LOXODON-HIERARCH",
        ability: ActivatedAbility {
            id: "sacrifice-source-regenerate-controller-creatures",
            mana_cost: ManaCost::with_colors(0, [Color::Green, Color::White]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: true,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::RegenerateControllerCreatures],
        },
    }
}
