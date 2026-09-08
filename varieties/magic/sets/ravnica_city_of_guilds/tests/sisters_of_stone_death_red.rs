//! Red contract for Sisters of Stone Death's source-linked combat abilities.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn sisters_of_stone_death_requires_the_full_source_linked_combat_definition() {
    assert_eq!(
        executable_definition_id_for_collector(231),
        Ok("RAV-SISTERS-OF-STONE-DEATH")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SISTERS-OF-STONE-DEATH")
        .expect("Sisters of Stone Death definition exists");
    assert_eq!(definition.name, "Sisters of Stone Death");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Black, Color::Black, Color::Green, Color::Green])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Black, Color::Green])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(7), Some(5)));
    for rule in [
        "legendary-permanent",
        "target-creature-must-block-source-this-turn-if-able",
        "exile-target-creature-blocking-or-blocked-by-source",
        "return-source-linked-exiled-creature-under-controller-control",
    ] {
        assert!(definition.supported_rules.contains(&rule), "missing {rule}");
    }
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
