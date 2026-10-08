//! Candidate-owned definition and bindings for Twilight Drover.
//!
//! The card-definition schema does not yet represent normal creature
//! subtypes, so the printed Spirit subtype is documented by the task but is
//! not asserted in `CardDefinition`. The white Spirit token is fully typed.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostBinding, ActivatedCounterCost,
    ActivatedCounterCostTarget, CardDefinition, CardType, Color, CounterKind, Effect,
    GeneralizedActivatedAbilityCost, ManaCost, TokenSpec, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TWILIGHT-DROVER",
        name: "Twilight Drover",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "creature-token-leaves-battlefield-plus-one-counter",
            "activated-remove-counter-create-two-flying-spirits",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-TWILIGHT-DROVER",
        ability: TriggeredAbility {
            id: "creature-token-leaves-plus-one-counter",
            condition: TriggerCondition::AnotherCreatureTokenLeavesBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AddPlusOneCounterToSource],
        },
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-TWILIGHT-DROVER",
        ability: ActivatedAbility {
            id: "create-two-flying-spirits",
            mana_cost: ManaCost::with_colors(2, [Color::White]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::white_spirit(),
                count: 2,
            }],
        },
    }
}

#[must_use]
pub(crate) fn activated_ability_cost() -> ActivatedAbilityCostBinding {
    ActivatedAbilityCostBinding {
        card_definition: "RAV-TWILIGHT-DROVER",
        ability_id: "create-two-flying-spirits",
        cost: GeneralizedActivatedAbilityCost {
            counter_removals: vec![ActivatedCounterCost {
                target: ActivatedCounterCostTarget::Source,
                counter: CounterKind::PlusOnePlusOne,
                amount: 1,
            }],
            ..GeneralizedActivatedAbilityCost::default()
        },
    }
}
