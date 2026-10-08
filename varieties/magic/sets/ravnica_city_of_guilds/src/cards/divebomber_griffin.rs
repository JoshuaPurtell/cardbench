//! Candidate-owned RAV Divebomber Griffin definition and sacrifice ability.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DIVEBOMBER-GRIFFIN",
        name: "Divebomber Griffin",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "sacrifice-source",
            "attacking-or-blocking-creature-target",
            "damage",
        ],
        power: Some(3),
        toughness: Some(2),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-DIVEBOMBER-GRIFFIN",
        ability: ActivatedAbility {
            id: "sacrifice-deal-three-to-attacker-or-blocker",
            mana_cost: ManaCost::new(0),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: true,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::AttackingOrBlockingCreature],
            effects: vec![Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::AttackingOrBlockingCreature,
            }],
        },
    }
}
