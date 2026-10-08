//! Candidate-owned RAV #246 Gaze of the Gorgon definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, HybridManaSymbol, ManaCost};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GAZE-OF-THE-GORGON",
        name: "Gaze of the Gorgon",
        set_code: "RAV",
        mana_cost: ManaCost::with_hybrid(
            3,
            [],
            [HybridManaSymbol {
                first: Color::Black,
                second: Color::Green,
            }],
        ),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "targeted-regeneration-shield",
            "delayed-end-of-combat-block-history-destruction",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RegenerateTargetCreatureAndScheduleCombatHistoryDestruction],
    }
}
