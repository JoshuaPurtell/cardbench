//! Candidate-owned RAV Grave-Shell Scarab definition and activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, Keyword, ManaCost,
};

/// RAV #211 is a black-green 4/4 with Dredge 1.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GRAVE-SHELL-SCARAB",
        name: "Grave-Shell Scarab",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "dredge",
            "sacrifice-source-draw",
        ],
        power: Some(4),
        toughness: Some(4),
        keywords: vec![Keyword::Dredge(1)],
        effects: vec![],
    }
}

/// `{1}, Sacrifice Grave-Shell Scarab: Draw a card.`
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "sacrifice-source-draw",
        mana_cost: ManaCost::new(1),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: true,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![],
        effects: vec![Effect::DrawController],
    }
}
