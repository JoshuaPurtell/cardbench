//! Red regression for the reusable triggered-ability scheduling boundary.

use cardbench_magic_engine::{
    AbilityActivation, Effect, Game, GameEvent, PlayerId, Target, TriggerCondition, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn belltower_damage_trigger_is_bound_through_the_generic_scheduler() {
    let ability = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-BELLTOWER-SPHINX")
        .expect("Belltower Sphinx damage trigger binding exists")
        .ability;

    assert_eq!(ability.condition, TriggerCondition::ReceivesDamage);
    assert!(ability.targets.is_empty());
    assert_eq!(
        ability.effects,
        [Effect::MillSourceControllerFromSourceDamage],
        "the source controller and damage amount must be materialized by the deferred trigger scheduler"
    );
}

#[test]
fn belltower_damage_mills_damage_source_controller_without_target_choice() {
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
    let sphinx = game
        .put_on_battlefield(PlayerId(0), "RAV-BELLTOWER-SPHINX")
        .expect("Belltower Sphinx enters");
    let fangtail = game
        .put_on_battlefield(PlayerId(1), "RAV-VIASHINO-FANGTAIL")
        .expect("Viashino Fangtail enters");
    game.set_entered_turn_for_setup(fangtail, 0)
        .expect("Fangtail predates the measured turn");
    let milled = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("opponent library card enters");
    let untouched = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Library)
        .expect("Sphinx controller library card enters");

    game.begin_game().expect("game starts");
    game.pass_priority(PlayerId(0))
        .expect("Fangtail controller receives priority");
    game.activate_ability(
        PlayerId(1),
        AbilityActivation {
            source: fangtail,
            ability_id: "tap-deal-one-to-player-or-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(sphinx)],
        },
    )
    .expect("Fangtail targets Belltower Sphinx");
    game.pass_priority(PlayerId(1))
        .expect("Fangtail controller passes");
    game.pass_priority(PlayerId(0))
        .expect("Fangtail ability resolves and queues the trigger");

    assert!(
        game.view_for_player(PlayerId(0))
            .expect("Sphinx controller view")
            .triggered_ability_target_choice
            .is_none()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == sphinx && *ability == "damage-source-controller-mill-that-many"
    )));
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");

    println!(
        "Belltower scheduler trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(milled), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(untouched), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == sphinx && *ability == "damage-source-controller-mill-that-many"
    )));
    game.validate_invariants()
        .expect("Belltower scheduler trace preserves invariants");
}
