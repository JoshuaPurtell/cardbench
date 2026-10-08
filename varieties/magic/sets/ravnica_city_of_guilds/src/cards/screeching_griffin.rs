//! CardBench-authored RAV Screeching Griffin identity and activated ability binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
};

/// Printed characteristics for the RAV white creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SCREECHING-GRIFFIN",
        name: "Screeching Griffin",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "activated-prevent-target-blocking-source",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

/// Candidate-owned binding for the red source-relative blocking restriction.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "prevent-target-blocking-griffin",
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Creature],
        effects: vec![Effect::PreventTargetBlockingSourceUntilEndOfTurn],
    }
}
