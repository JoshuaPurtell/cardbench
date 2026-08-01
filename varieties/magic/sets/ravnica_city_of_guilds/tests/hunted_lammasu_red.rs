//! Red discovery contract for the White source-lane Hunted Lammasu path.

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn hunted_lammasu_requires_flying_and_a_targeted_opponent_horror_etb() {
    let lammasu = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-LAMMASU")
        .expect("Hunted Lammasu definition exists");
    assert_eq!(lammasu.mana_cost, ManaCost::with_colors(2, [Color::White, Color::White]));
    assert_eq!(lammasu.card_types, [CardType::Creature].into());
    assert_eq!((lammasu.power, lammasu.toughness), (Some(5), Some(5)));
    assert_eq!(lammasu.keywords, [Keyword::Flying]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&lammasu.id));
    assert!(lammasu
        .supported_rules
        .contains(&"etb-targeted-opponent-horror-token"));
}
