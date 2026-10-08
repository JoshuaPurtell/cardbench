//! Candidate-owned Barbarian Riftcutter definition and land-destruction ability.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BARBARIAN-RIFTCUTTER",
        name: "Barbarian Riftcutter",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "sacrifice-source-destroy-land",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

/// Pay red and sacrifice this creature to destroy a target land.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-BARBARIAN-RIFTCUTTER",
        ability: ActivatedAbility {
            id: "sacrifice-destroy-target-land",
            mana_cost: ManaCost::with_colors(0, [Color::Red]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: true,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Land],
            effects: vec![Effect::DestroyTargetLand],
        },
    }
}
