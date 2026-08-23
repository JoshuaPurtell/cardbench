//! Red regression for Induce Paranoia's spent-blue counter/mill behavior.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn induce_paranoia_requires_spent_blue_countered_spell_mill_behavior() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INDUCE-PARANOIA")
        .expect("Induce Paranoia definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Instant].into_iter().collect()
    );
    assert!(
        definition
            .supported_rules
            .contains(&"counter-spell-then-mill-controller-by-mana-value-if-blue-spent")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
