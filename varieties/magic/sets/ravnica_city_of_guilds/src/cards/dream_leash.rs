//! Candidate-owned RAV #45 Dream Leash identity and tapped-permanent Aura effect.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Effect, ManaCost, TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DREAM-LEASH",
        name: "Dream Leash",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "aura-enchant-permanent-source-controller-control",
            "aura-target-must-be-tapped-when-cast",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTappedTarget {
            target: TargetRequirement::Permanent,
            changes: vec![ContinuousChange::ChangeControllerToSourceController],
        }],
    }
}
