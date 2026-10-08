//! Candidate-owned definition and activated ability for RAV Caregiver.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CAREGIVER",
        name: "Caregiver",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "sacrifice-source-targeted-one-damage-prevention",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-CAREGIVER",
        ability: ActivatedAbility {
            id: "prevent-one-damage",
            mana_cost: ManaCost::with_colors(0, [Color::White]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: true,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::PlayerOrCreature],
            effects: vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
        },
    }
}
