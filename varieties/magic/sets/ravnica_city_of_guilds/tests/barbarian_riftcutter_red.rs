use cardbench_magic_engine::{AbilityActivation, Color, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn barbarian_riftcutter_declares_its_sacrifice_land_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BARBARIAN-RIFTCUTTER")
        .expect("Barbarian Riftcutter exists");
    assert!(definition.colors.contains(&Color::Red));
    assert!(
        definition
            .supported_rules
            .contains(&"sacrifice-source-destroy-land")
    );
}

#[test]
fn barbarian_riftcutter_sacrifices_and_destroys_a_target_land() {
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
        .put_on_battlefield(PlayerId(0), "RAV-BARBARIAN-RIFTCUTTER")
        .expect("Riftcutter enters");
    let land = game
        .put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
        .expect("target land enters");
    game.set_entered_turn_for_setup(source, 0)
        .expect("old source");
    game.begin_game().expect("game starts");
    game.add_mana_from_action(PlayerId(0), Color::Red, 1)
        .expect("red activation mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "sacrifice-destroy-target-land",
            sacrifice_sources: vec![source],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(land)],
        },
    )
    .expect("Riftcutter activation");
    assert_eq!(game.zone_of(source), Some(Zone::Graveyard));
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("ability resolves");
    assert_eq!(game.zone_of(land), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { card, .. } if *card == land
    )));
}
