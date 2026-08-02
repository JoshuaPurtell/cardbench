//! Red discovery contract for Vedalken Entrancer's milling activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn vedalken_entrancer_requires_tap_mana_target_player_mill_two_activation() {
    let entrancer = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VEDALKEN-ENTRANCER")
        .expect("Vedalken Entrancer definition exists");

    assert_eq!(entrancer.name, "Vedalken Entrancer");
    assert_eq!(entrancer.mana_cost, ManaCost::with_colors(3, [Color::Blue]));
    assert_eq!(entrancer.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(entrancer.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((entrancer.power, entrancer.toughness), (Some(1), Some(4)));
    assert_eq!(entrancer.keywords, [Keyword::Defender]);
    assert!(entrancer.effects.is_empty());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&entrancer.id));
    assert!(
        entrancer
            .supported_rules
            .contains(&"tap-blue-target-player-mill-two")
    );

    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-VEDALKEN-ENTRANCER")
        .expect("Vedalken Entrancer mill binding exists");
    assert_eq!(binding.ability.id, "tap-blue-mill-two");
    assert_eq!(binding.ability.mana_cost, ManaCost::with_colors(0, [Color::Blue]));
    assert!(binding.ability.tap_cost);
    assert_eq!(binding.ability.targets, [TargetRequirement::Player]);
    assert_eq!(binding.ability.effects, [Effect::MillTargetPlayer { count: 2 }]);
}
