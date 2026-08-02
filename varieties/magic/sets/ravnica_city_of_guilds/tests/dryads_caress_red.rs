//! Red contract for Dryad's Caress: its graveyard creature count and selected
//! creature-card return must both survive spell targeting and resolution.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dryads_caress_requires_graveyard_count_and_creature_return_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DRYADS-CARESS")
        .expect("Dryad's Caress exists");
    assert!(
        definition
            .supported_rules
            .contains(&"graveyard-creature-count-life-gain-and-target-return"),
        "Dryad's Caress must expose both printed graveyard instructions"
    );
    assert!(
        definition.effects.len() >= 2,
        "Dryad's Caress needs a count-based life gain and an exact creature-card return"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Dryad's Caress is not full fidelity until both effects are represented"
    );
}
