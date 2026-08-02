//! Red discovery contract for Bathe in Light's policy-chosen protection color.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn bathe_in_light_has_its_chosen_color_team_protection_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BATHE-IN-LIGHT")
        .expect("Bathe in Light definition exists");
    assert_eq!(definition.name, "Bathe in Light");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"chosen-color-controller-creature-protection")
    );
}
