use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, PlayerId, Target,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn wojek_embermage_is_executable_with_its_radiance_tap_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOJEK-EMBERMAGE")
        .expect("Wojek Embermage exists");
    assert_eq!(definition.name, "Wojek Embermage");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(1)));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-radiance-one-damage")
    );
}

#[test]
fn wojek_embermage_radiance_damages_the_target_and_shared_color_creature() {
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
        .put_on_battlefield(PlayerId(0), "RAV-WOJEK-EMBERMAGE")
        .expect("Embermage enters");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target enters");
    let shared = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-RECRUIT")
        .expect("shared-color creature enters");
    game.set_entered_turn_for_setup(source, 0)
        .expect("old source");
    game.set_entered_turn_for_setup(target, 0)
        .expect("old target");
    game.set_entered_turn_for_setup(shared, 0)
        .expect("old shared creature");
    game.begin_game().expect("game starts");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "tap-radiance-one-damage",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("radiance ability activates");
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("ability resolves");
    assert_eq!(game.object(target).expect("target remains").damage, 1);
    assert_eq!(game.object(shared).expect("shared remains").damage, 1);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source: resolved, ability }
            if *resolved == source && *ability == "tap-radiance-one-damage"
    )));
}
