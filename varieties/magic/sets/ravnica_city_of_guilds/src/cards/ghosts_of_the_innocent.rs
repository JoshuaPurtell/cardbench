//! Candidate-owned RAV Ghosts of the Innocent definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DamageReplacementEffect, DamageReplacementEffectBinding,
    ManaCost,
};

/// Printed identity and base characteristics for the global damage reducer.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GHOSTS-OF-THE-INNOCENT",
        name: "Ghosts of the Innocent",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "static-global-damage-amount-halving",
            "affected-player-damage-replacement-ordering",
        ],
        power: Some(4),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

/// Live Ghosts instances halve each represented damage packet.
#[must_use]
pub(crate) fn damage_replacement_binding() -> DamageReplacementEffectBinding {
    DamageReplacementEffectBinding {
        source_definition: "RAV-GHOSTS-OF-THE-INNOCENT",
        effect: DamageReplacementEffect::HalveDamage,
    }
}
