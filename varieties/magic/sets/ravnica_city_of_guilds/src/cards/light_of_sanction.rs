//! Candidate-owned definition and friendly-source damage prevention for Light of Sanction.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Keyword, ManaCost,
    StaticContinuousEffectBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LIGHT-OF-SANCTION",
        name: "Light of Sanction",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "static-prevent-friendly-source-damage",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn static_continuous_effect() -> StaticContinuousEffectBinding {
    StaticContinuousEffectBinding {
        card_definition: "RAV-LIGHT-OF-SANCTION",
        change: ContinuousChange::ControlledCreaturesAddKeyword(
            Keyword::PreventDamageFromControlledSources,
        ),
    }
}
