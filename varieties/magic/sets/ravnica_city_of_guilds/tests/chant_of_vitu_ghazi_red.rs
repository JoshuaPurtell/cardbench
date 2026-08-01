//! Red discovery contract for Chant of Vitu-Ghazi's Convoke life-gain effect.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn chant_of_vitu_ghazi_requires_dynamic_convoke_life_gain() {
    let chant = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CHANT-OF-VITU-GHAZI")
        .expect("Chant of Vitu-Ghazi definition exists");

    assert_eq!(chant.name, "Chant of Vitu-Ghazi");
    assert_eq!(
        chant.mana_cost,
        ManaCost::with_colors(6, [Color::White, Color::White])
    );
    assert_eq!(chant.colors, BTreeSet::from([Color::White]));
    assert_eq!(chant.card_types, BTreeSet::from([CardType::Instant]));
    assert_eq!(chant.keywords, vec![Keyword::Convoke]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&chant.id));
    assert!(
        chant
            .supported_rules
            .contains(&"convoke-dynamic-battlefield-life-gain")
    );
}
