//! Red discovery contract for the White source-lane Hour of Reckoning path.

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn hour_of_reckoning_requires_convoke_and_global_nontoken_creature_destruction() {
    let hour = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HOUR-OF-RECKONING")
        .expect("Hour of Reckoning definition exists");
    assert_eq!(
        hour.mana_cost,
        ManaCost::with_colors(4, [Color::White, Color::White, Color::White])
    );
    assert_eq!(hour.card_types, [CardType::Sorcery].into());
    assert_eq!(hour.keywords, [Keyword::Convoke]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&hour.id));
    assert!(hour
        .supported_rules
        .contains(&"destroy-all-nontoken-creatures"));
}
