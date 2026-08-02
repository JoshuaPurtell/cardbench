//! Red discovery contract for Galvanic Arc's attached activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn galvanic_arc_requires_a_creature_aura_that_grants_a_tap_damage_ability() {
    let arc = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GALVANIC-ARC")
        .expect("Galvanic Arc definition exists");

    assert_eq!(arc.name, "Galvanic Arc");
    assert_eq!(arc.mana_cost, ManaCost::with_colors(2, [Color::Red]));
    assert_eq!(arc.colors, BTreeSet::from([Color::Red]));
    assert_eq!(arc.card_types, BTreeSet::from([CardType::Enchantment]));
    assert!(
        arc.supported_rules
            .contains(&"aura-grants-tap-three-damage-to-player-or-creature"),
        "the printed ability must remain an attachment-scoped, stack-backed activation",
    );
}
