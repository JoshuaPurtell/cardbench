//! Red source contract for Galvanic Arc's printed identity and rules scope.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn galvanic_arc_is_a_red_three_mana_aura_with_its_printed_rules_scope() {
    let arc = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GALVANIC-ARC")
        .expect("Galvanic Arc definition exists");

    assert_eq!(arc.name, "Galvanic Arc");
    assert_eq!(arc.mana_cost, ManaCost::with_colors(2, [Color::Red]));
    assert_eq!(arc.colors, BTreeSet::from([Color::Red]));
    assert_eq!(arc.card_types, BTreeSet::from([CardType::Enchantment]));
    assert!(arc.supported_rules.contains(&"aura-enchant-creature"));
    assert!(
        arc.supported_rules
            .contains(&"etb-three-damage-to-target-creature-or-player")
    );
    assert!(
        arc.supported_rules
            .contains(&"attached-creature-first-strike")
    );
}
