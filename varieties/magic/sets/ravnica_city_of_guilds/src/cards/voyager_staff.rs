//! Candidate-owned RAV Voyager Staff definition and activated ability.

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Effect, ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

/// Printed identity and one-mana artifact casting cost.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VOYAGER-STAFF",
        name: "Voyager Staff",
        set_code: "RAV",
        mana_cost: ManaCost::new(1),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-artifact-casting",
            "sacrifice-linked-exile-target-creature-until-end-step",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// `{2}, Sacrifice Voyager Staff`: temporarily exile target creature.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "sacrifice-linked-exile-target-creature-until-end-step",
        mana_cost: ManaCost::new(2),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: true,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Creature],
        effects: vec![Effect::ExileTargetCreatureUntilEndStep],
    }
}
