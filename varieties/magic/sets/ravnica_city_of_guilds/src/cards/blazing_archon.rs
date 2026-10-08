//! Candidate-owned RAV Blazing Archon definition and attack restriction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Keyword, ManaCost, StaticAttackRestriction,
    StaticAttackRestrictionBinding,
};

/// Printed identity and live flying/attack-prohibition characteristics.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BLAZING-ARCHON",
        name: "Blazing Archon",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(6, [Color::White, Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "static-opponents-cannot-attack-controller",
        ],
        power: Some(5),
        toughness: Some(6),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

/// The live source prohibits opposing attackers from attacking its controller.
#[must_use]
pub(crate) fn static_attack_restriction_binding() -> StaticAttackRestrictionBinding {
    StaticAttackRestrictionBinding {
        card_definition: "RAV-BLAZING-ARCHON",
        restriction: StaticAttackRestriction::OpponentsCannotAttackController,
    }
}
