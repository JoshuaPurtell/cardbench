//! Candidate-owned rules definition for War-Torch Goblin.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-WAR-TORCH-GOBLIN",
        name: "War-Torch Goblin",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "sacrifice-source",
            "targeted-damage",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn sacrifice_deal_two_to_blocker_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "sacrifice-deal-two-to-blocker",
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: true,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::BlockingCreature],
        effects: vec![Effect::DealDamage {
            amount: 2,
            target: TargetRequirement::BlockingCreature,
        }],
    }
}
