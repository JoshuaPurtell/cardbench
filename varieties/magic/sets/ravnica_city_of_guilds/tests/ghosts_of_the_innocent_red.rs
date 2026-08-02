//! Red discovery contract for Ghosts of the Innocent's global damage reduction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn ghosts_of_the_innocent_requires_a_global_damage_amount_replacement_slice() {
    let ghosts = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GHOSTS-OF-THE-INNOCENT")
        .expect("Ghosts of the Innocent definition exists");

    assert_eq!(ghosts.name, "Ghosts of the Innocent");
    assert_eq!(
        ghosts.mana_cost,
        ManaCost::with_colors(5, [Color::White, Color::White])
    );
    assert_eq!(ghosts.colors, BTreeSet::from([Color::White]));
    assert_eq!(ghosts.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((ghosts.power, ghosts.toughness), (Some(4), Some(5)));
    assert!(ghosts.keywords.is_empty());
    assert!(ghosts.effects.is_empty());
    assert!(
        ghosts
            .supported_rules
            .contains(&"static-global-damage-amount-halving")
    );
    assert_eq!(
        executable_definition_id_for_collector(20),
        Ok("RAV-GHOSTS-OF-THE-INNOCENT")
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-GHOSTS-OF-THE-INNOCENT"),
        "global replacement-order coverage remains an explicit bounded compatibility scope"
    );
}
