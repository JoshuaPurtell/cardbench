//! Red discovery contract for Followed Footsteps' copied-token upkeep trigger.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn followed_footsteps_requires_attached_creature_token_copy_substrate() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FOLLOWED-FOOTSTEPS")
        .expect("Followed Footsteps definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(definition.name, "Followed Footsteps");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Blue, Color::Blue])
    );
    assert_eq!(definition.colors, [Color::Blue].into());
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into(),
        "the full fixture treats the source as an Aura-bound enchantment"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"controller-upkeep-attached-creature-token-copy")
    );
}
