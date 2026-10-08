//! Candidate-owned RAV Indentured Oaf identity and static damage prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-INDENTURED-OAF",
        name: "Indentured Oaf",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "prevent-damage-by-this-source-to-red-creatures",
        ],
        power: Some(4),
        toughness: Some(3),
        keywords: vec![Keyword::PreventDamageToCreaturesOfColor(Color::Red)],
        effects: vec![],
    }
}
