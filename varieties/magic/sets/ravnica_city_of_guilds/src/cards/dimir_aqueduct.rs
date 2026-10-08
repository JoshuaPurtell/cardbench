//! Candidate-owned RAV Dimir Aqueduct definition and land bindings.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, Effect, LandEntryBinding,
    ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, TargetRequirement,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DIMIR-AQUEDUCT",
        name: "Dimir Aqueduct",
        set_code: "RAV",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "enters-tapped",
            "etb-return-controlled-land",
            "fixed-blue-black-mana-bundle",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn land_entry_binding() -> LandEntryBinding {
    LandEntryBinding {
        card_definition: "RAV-DIMIR-AQUEDUCT",
        enters_tapped: true,
        optional_life_payment: None,
    }
}

#[must_use]
pub(crate) fn mana_ability() -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition: "RAV-DIMIR-AQUEDUCT",
        ability: ActivatedManaAbility {
            id: "dimir-aqueduct-ub",
            tap_cost: true,
            output: ManaAbilityOutput::Bundle(ManaBundle::new([
                (Color::Blue, 1),
                (Color::Black, 1),
            ])),
            amount: 0,
            life_payment: None,
            controller_damage: None,
        },
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-DIMIR-AQUEDUCT",
        ability: TriggeredAbility {
            id: "return-controlled-land",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::ControlledLand],
            effects: vec![Effect::ReturnControlledLandToHand],
        },
    }
}
