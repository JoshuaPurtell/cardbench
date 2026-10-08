//! Candidate-owned RAV Ursapine identity and activated pump ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-URSAPINE",
        name: "Ursapine",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-target-pump",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

/// `{G}: Target creature gets +1/+1 until end of turn.`
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-URSAPINE",
        ability: ActivatedAbility {
            id: "ursapine-target-pump",
            mana_cost: ManaCost::with_colors(0, [Color::Green]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            }],
        },
    }
}
