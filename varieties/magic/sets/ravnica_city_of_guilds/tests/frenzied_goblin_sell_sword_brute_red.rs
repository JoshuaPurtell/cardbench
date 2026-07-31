//! Red discovery probes for Frenzied Goblin and Sell-Sword Brute.
//!
//! These assertions deliberately fail until the target-bearing triggered-ability
//! substrate can represent both printed rules texts without approximation.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn frenzied_goblin_requires_its_optional_paid_attack_trigger() {
    let goblin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FRENZIED-GOBLIN")
        .expect("Frenzied Goblin definition exists");
    println!(
        "Frenzied Goblin discovery: full_fidelity={}, supported_rules={:?}",
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&goblin.id),
        goblin.supported_rules
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&goblin.id),
        "Frenzied Goblin needs its attack trigger, optional {{R}} payment, creature target, and cannot-block-until-end-of-turn effect"
    );
}

#[test]
fn sell_sword_brute_requires_its_death_damage_trigger() {
    let brute = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELL-SWORD-BRUTE")
        .expect("Sell-Sword Brute definition exists");
    println!(
        "Sell-Sword Brute discovery: full_fidelity={}, supported_rules={:?}",
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&brute.id),
        brute.supported_rules
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&brute.id),
        "Sell-Sword Brute needs its dies trigger and two-damage player-or-creature target"
    );
}
