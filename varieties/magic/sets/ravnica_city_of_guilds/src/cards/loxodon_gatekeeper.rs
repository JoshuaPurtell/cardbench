//! Candidate-owned RAV Loxodon Gatekeeper definition and entry restriction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ManaCost, StaticEntryRestriction,
    StaticEntryRestrictionBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LOXODON-GATEKEEPER",
        name: "Loxodon Gatekeeper",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "static-opponents-artifacts-creatures-lands-enter-tapped",
        ],
        power: Some(2),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn static_entry_restriction_binding() -> StaticEntryRestrictionBinding {
    StaticEntryRestrictionBinding {
        card_definition: "RAV-LOXODON-GATEKEEPER",
        restriction: StaticEntryRestriction::OpponentsArtifactsCreaturesAndLandsEnterTapped,
    }
}
