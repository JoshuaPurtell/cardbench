//! Red discovery contract for Flame Fusillade's temporary ability grant.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn flame_fusillade_requires_a_controller_creature_snapshot_tap_damage_grant() {
    let fusillade = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FLAME-FUSILLADE")
        .expect("Flame Fusillade definition exists");

    assert_eq!(fusillade.name, "Flame Fusillade");
    assert_eq!(fusillade.mana_cost, ManaCost::with_colors(3, [Color::Red]));
    assert_eq!(fusillade.colors, BTreeSet::from([Color::Red]));
    assert_eq!(fusillade.card_types, BTreeSet::from([CardType::Sorcery]));
    assert!(
        fusillade
            .supported_rules
            .contains(&"controller-creature-tap-one-damage-grant-until-end-of-turn"),
        "the temporary ability must snapshot only the caster's current creatures and use normal stack-backed activation",
    );
}
