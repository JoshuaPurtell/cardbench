//! Candidate-owned RAV Blockbuster definition and sacrifice activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
};

/// Printed identity and executable enchantment effect.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BLOCKBUSTER",
        name: "Blockbuster",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-enchantment-casting",
            "sacrifice-source-deal-three-to-tapped-creatures-and-players",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// Sacrifice the enchantment as an activation cost, then hit tapped
/// creatures and each player for three damage.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-BLOCKBUSTER",
        ability: ActivatedAbility {
            id: "sacrifice-source-deal-three-to-tapped-creatures-and-players",
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: true,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::DealDamageToEachTappedCreatureAndPlayer { amount: 3 }],
        },
    }
}
