use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, Keyword, PlayerId, Target,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn thundersong_trumpeter_is_executable_with_its_combat_restriction_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-THUNDERSONG-TRUMPETER")
        .expect("Thundersong Trumpeter exists");
    assert_eq!(definition.name, "Thundersong Trumpeter");
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(1)));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-prevent-target-combat")
    );
}

#[test]
fn thundersong_trumpeter_taps_and_marks_a_target_as_unable_to_attack_or_block() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-THUNDERSONG-TRUMPETER")
        .expect("Trumpeter enters");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target enters");
    game.set_entered_turn_for_setup(source, 0)
        .expect("old source");
    game.begin_game().expect("game starts");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "tap-prevent-target-combat",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("restriction ability activates");
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1))
        .expect("restriction resolves");
    assert!(game.object(source).expect("source remains").tapped);
    assert!(
        game.characteristics(target)
            .expect("target characteristics")
            .keywords
            .contains(&Keyword::CannotAttackOrBlock)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source: effect_source, target: effect_target, .. }
            if *effect_source == source && *effect_target == target
    )));
}
