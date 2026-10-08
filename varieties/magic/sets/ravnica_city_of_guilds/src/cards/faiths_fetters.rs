//! Candidate-owned RAV Faith's Fetters definition and ETB life-gain binding.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Effect, Keyword, ManaCost,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FAITHS-FETTERS",
        name: "Faith's Fetters",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "aura-enchant-permanent",
            "etb-gain-four-life",
            "attached-permanent-combat-and-activation-restriction",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Permanent,
            changes: vec![
                ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock),
                ContinuousChange::SuppressNonManaActivatedAbilities,
            ],
        }],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-FAITHS-FETTERS",
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
