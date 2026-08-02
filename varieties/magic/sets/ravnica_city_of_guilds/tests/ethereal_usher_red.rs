//! Red discovery contract for Ethereal Usher's complete printed ability set.
//!
//! The engine already owns stack-backed Transmute and source-relative combat
//! evasion. This probe records that the executable chassis has not yet bound
//! either supported ability to the card.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn ethereal_usher_requires_its_tap_evasion_and_transmute_abilities_for_full_fidelity() {
    let usher = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ETHEREAL-USHER")
        .expect("Ethereal Usher definition exists");

    assert_eq!(usher.name, "Ethereal Usher");
    assert_eq!(usher.mana_cost, ManaCost::with_colors(5, [Color::Blue]));
    assert_eq!(usher.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(usher.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((usher.power, usher.toughness), (Some(2), Some(3)));
    assert_eq!(
        usher.keywords,
        [Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue],
        ))]
    );
    assert!(
        usher
            .supported_rules
            .contains(&"activated-target-unblockable-until-end-of-turn")
    );
    assert!(usher.supported_rules.contains(&"transmute"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&usher.id));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-ETHEREAL-USHER"
            && binding.ability.id == "tap-target-unblockable"
    }));
}
