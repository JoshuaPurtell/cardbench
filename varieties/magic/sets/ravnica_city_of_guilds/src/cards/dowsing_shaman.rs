//! Candidate-owned RAV Dowsing Shaman identity and enchantment-return activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DOWSING-SHAMAN",
        name: "Dowsing Shaman",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "return-target-enchantment-from-graveyard",
        ],
        power: Some(3),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-DOWSING-SHAMAN",
        ability: ActivatedAbility {
            id: "return-target-enchantment-from-graveyard",
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::EnchantmentCardInControllerGraveyard],
            effects: vec![Effect::ReturnTargetEnchantmentCardToHand],
        },
    }
}
