//! Red discovery contract for Tunnel Vision's named-card library traversal.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn tunnel_vision_requires_named_card_target_library_traversal() {
    assert_eq!(
        executable_definition_id_for_collector(72),
        Ok("RAV-TUNNEL-VISION")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TUNNEL-VISION")
        .expect("Tunnel Vision definition exists");

    assert_eq!(definition.name, "Tunnel Vision");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(5, [Color::Blue])
    );
    assert_eq!(definition.colors, [Color::Blue].into());
    assert_eq!(definition.card_types, [CardType::Sorcery].into());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    for rule in [
        "target-player-library-named-card-traversal",
        "named-card-placed-on-library-top-before-shuffle",
        "other-revealed-cards-move-to-target-graveyard",
        "target-library-shuffled-after-named-traversal",
    ] {
        assert!(definition.supported_rules.contains(&rule), "missing {rule}");
    }
}
