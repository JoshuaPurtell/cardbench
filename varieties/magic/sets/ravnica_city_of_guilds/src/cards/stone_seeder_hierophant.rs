//! Candidate-owned RAV Stone-Seeder Hierophant identity and abilities.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-STONE-SEEDER-HIEROPHANT",
        name: "Stone-Seeder Hierophant",
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
            "controlled-land-entry-untaps-source",
            "tap-untap-target-land",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// {T}: Untap target land.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-STONE-SEEDER-HIEROPHANT",
        ability: ActivatedAbility {
            id: "untap-target-land",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Land],
            effects: vec![Effect::UntapTargetLand],
        },
    }
}

/// Whenever a land enters under your control, untap this creature.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-STONE-SEEDER-HIEROPHANT",
        ability: TriggeredAbility {
            id: "landfall-untap-source",
            condition: TriggerCondition::ControlledLandEntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::UntapSource],
        },
    }
}
