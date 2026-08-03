//! Red discovery contract for Eye of the Storm's exiled-card copy/cast loop.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn eye_of_the_storm_requires_exiled_spell_copy_and_free_cast_substrate() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-EYE-OF-THE-STORM")
        .expect("Eye of the Storm definition exists");

    assert_eq!(definition.name, "Eye of the Storm");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(5, [Color::Blue, Color::Blue])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"any-player-instant-sorcery-cast-exile-and-copy")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"cast-exiled-spell-copies-without-paying-mana")
    );
}
