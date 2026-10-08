//! Candidate-owned RAV #251 Privileged Position identity and hexproof effect.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, HybridManaSymbol, Keyword, ManaCost,
    StaticContinuousEffectBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PRIVILEGED-POSITION",
        name: "Privileged Position",
        set_code: "RAV",
        mana_cost: ManaCost::with_hybrid(
            2,
            [],
            [
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
            ],
        ),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "hybrid-cost-casting",
            "other-controlled-permanents-have-hexproof",
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
        card_definition: "RAV-PRIVILEGED-POSITION",
        change: ContinuousChange::OtherControlledPermanentsAddKeyword(Keyword::Hexproof),
    }
}
