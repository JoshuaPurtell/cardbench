//! Candidate-owned RAV Conclave's Blessing definition.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Effect, Keyword, ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CONCLAVES-BLESSING",
        name: "Conclave's Blessing",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "aura-convoke-dynamic-other-controller-creature-toughness",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Convoke],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![
                ContinuousChange::ModifyPowerToughnessForEachOtherCreatureControlledBySource {
                    power_per_creature: 0,
                    toughness_per_creature: 2,
                },
            ],
        }],
    }
}
