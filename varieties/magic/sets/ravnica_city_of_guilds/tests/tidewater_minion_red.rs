//! Red discovery contract for Tidewater Minion's two exact activations.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Effect, Keyword, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn tidewater_minion_requires_target_permanent_untap_and_self_defender_removal() {
    let minion = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TIDEWATER-MINION")
        .expect("Tidewater Minion definition exists");

    assert_eq!(minion.name, "Tidewater Minion");
    assert_eq!(minion.mana_cost, ManaCost::with_colors(3, [Color::Blue, Color::Blue]));
    assert_eq!(minion.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(minion.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((minion.power, minion.toughness), (Some(4), Some(4)));
    assert_eq!(minion.keywords, [Keyword::Defender]);
    assert!(minion.effects.is_empty());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&minion.id));
    assert!(
        minion
            .supported_rules
            .contains(&"tap-untap-target-permanent-and-blue-lose-defender")
    );

    let bindings = rav_activated_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == "RAV-TIDEWATER-MINION")
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2);
    let untap = bindings
        .iter()
        .find(|binding| binding.ability.id == "tap-untap-target-permanent")
        .expect("target-permanent untap binding exists");
    assert!(untap.ability.tap_cost);
    assert_eq!(untap.ability.mana_cost, ManaCost::new(0));
    assert_eq!(untap.ability.targets, [TargetRequirement::Permanent]);
    assert_eq!(untap.ability.effects, [Effect::UntapTargetPermanent]);

    let lose_defender = bindings
        .iter()
        .find(|binding| binding.ability.id == "blue-lose-defender")
        .expect("self defender-removal binding exists");
    assert!(!lose_defender.ability.tap_cost);
    assert_eq!(
        lose_defender.ability.mana_cost,
        ManaCost::with_colors(0, [Color::Blue])
    );
    assert_eq!(lose_defender.ability.targets, []);
    assert_eq!(
        lose_defender.ability.effects,
        [Effect::RemoveSourceKeywordUntilEndOfTurn {
            keyword: Keyword::Defender,
        }]
    );
}
