use cardbench_magic_engine::{AbilityActivation, Game, GameEvent, PlayerId, Target};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn coalhauler_swine_damage_trigger_stacks_then_deals_that_much_to_each_player() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let swine = game
        .put_on_battlefield(PlayerId(0), "RAV-COALHAULER-SWINE")
        .expect("Coalhauler Swine enters");
    let fangtail = game
        .put_on_battlefield(PlayerId(1), "RAV-VIASHINO-FANGTAIL")
        .expect("Viashino Fangtail enters");
    game.set_entered_turn_for_setup(fangtail, 0)
        .expect("fixture makes Fangtail long-controlled");
    game.begin_game().expect("game starts");
    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.activate_ability(
        PlayerId(1),
        AbilityActivation {
            source: fangtail,
            ability_id: "tap-deal-one-to-player-or-creature",
            sacrifice_sources: vec![],
            targets: vec![Target::Permanent(swine)],
        },
    )
    .expect("Fangtail targets Coalhauler Swine");
    game.pass_priority(PlayerId(1))
        .expect("Fangtail controller passes");
    game.pass_priority(PlayerId(0))
        .expect("Fangtail ability resolves");

    println!("Coalhauler Swine missing-trigger events: {:#?}", game.event_log);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == swine && *ability == "damage-each-player"
    )));

    game.pass_priority(PlayerId(0))
        .expect("Swine trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("Swine trigger resolves");
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 19);
    assert_eq!(game.player(PlayerId(1)).expect("player exists").life, 19);
}
