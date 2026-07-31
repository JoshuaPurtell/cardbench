//! Red discovery probes for the next easy creature/ETB/static slice.

use cardbench_magic_engine::{CardType, Keyword};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn sparkmage_apprentice_requires_targeted_etb_damage() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SPARKMAGE-APPRENTICE")
        .expect("Sparkmage Apprentice definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.supported_rules.contains(&"etb-targeted-damage"));
}

#[test]
fn hunted_dragon_requires_haste_flying_and_opponent_knight_etb() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-DRAGON")
        .expect("Hunted Dragon definition exists");
    assert_eq!(definition.card_types, [CardType::Creature].into_iter().collect());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.keywords.contains(&Keyword::Flying));
    assert!(definition.keywords.contains(&Keyword::Haste));
    assert!(definition
        .supported_rules
        .contains(&"etb-opponent-knight-tokens"));
}

#[test]
fn razia_requires_flying_first_strike_and_vigilance() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-RAZIA-BOROS-ARCHANGEL")
        .expect("Razia, Boros Archangel definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.keywords.contains(&Keyword::Flying));
    assert!(definition.keywords.contains(&Keyword::FirstStrike));
    assert!(definition.keywords.contains(&Keyword::Vigilance));
}
