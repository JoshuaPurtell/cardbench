use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-INDUCE-PARANOIA",
        name: "Induce Paranoia",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "counter-spell-then-mill-controller-by-mana-value-if-black-spent",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::CounterTargetPhysicalSpellThenMillItsControllerByManaValueIfManaColorSpent {
                color: Color::Black,
            },
        ],
    }
}
