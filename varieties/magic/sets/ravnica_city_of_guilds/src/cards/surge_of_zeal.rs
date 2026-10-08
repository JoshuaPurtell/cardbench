use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

use crate::{SET_CODE, colors, types};

/// Candidate-owned definition for Surge of Zeal.
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SURGE-OF-ZEAL",
        name: "Surge of Zeal",
        set_code: SET_CODE,
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
        colors: colors([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "radiance-grant-haste",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RadianceAddKeywordUntilEndOfTurn {
            keyword: Keyword::Haste,
        }],
    }
}
