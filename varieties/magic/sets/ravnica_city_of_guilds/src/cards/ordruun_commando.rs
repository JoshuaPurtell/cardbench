//! Candidate-owned card definition and damage-prevention activation for
//! RAV Ordruun Commando.

use std::collections::BTreeSet;

use cardbench_magic_engine::{ActivatedAbility, CardDefinition, CardType, Color, Effect, ManaCost};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-ORDRUUN-COMMANDO",
        name: "Ordruun Commando",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-prevent-one-damage-to-self",
        ],
        power: Some(4),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn prevent_one_damage_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "prevent-one-damage-to-self",
        mana_cost: ManaCost::with_colors(0, [Color::White]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![],
        effects: vec![Effect::AddSourceDamageShieldUntilEndOfTurn { amount: 1 }],
    }
}
