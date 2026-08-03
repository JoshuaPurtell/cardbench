//! Red discovery contract for Woodwraith Strangler's graveyard-exile regeneration.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn woodwraith_strangler_requires_graveyard_exile_regeneration() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOODWRAITH-STRANGLER")
        .expect("Woodwraith Strangler definition exists");

    assert_eq!(definition.name, "Woodwraith Strangler");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Green])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Black, Color::Green]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Creature])
    );
    assert_eq!((definition.power, definition.toughness), (Some(3), Some(2)));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Woodwraith Strangler cannot be complete while its graveyard-exile regeneration is absent"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"exile-controller-graveyard-creature-card-regenerate-source"),
        "Woodwraith Strangler must expose its graveyard-exile regeneration cost"
    );
}
