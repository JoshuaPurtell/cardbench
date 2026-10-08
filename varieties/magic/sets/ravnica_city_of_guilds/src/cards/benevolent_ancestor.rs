//! Candidate-owned definition and targeted prevention ability for
//! RAV Benevolent Ancestor.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
};

/// Printed identity and defensive creature characteristics.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BENEVOLENT-ANCESTOR",
        name: "Benevolent Ancestor",
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
            "defender",
            "tap-prevent-one-damage-to-target",
        ],
        power: Some(0),
        toughness: Some(4),
        keywords: vec![Keyword::Defender],
        effects: vec![],
    }
}

/// `{T}: Prevent the next 1 damage that would be dealt to target creature or
/// player this turn.`
#[must_use]
pub(crate) fn prevention_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "tap-prevent-one-damage",
        mana_cost: ManaCost::new(0),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::PlayerOrCreature],
        effects: vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
    }
}
