//! Red regression for the reusable triggered-ability scheduling boundary.

use cardbench_magic_engine::{
    AbilityActivation, Effect, Game, GameEvent, PlayerId, PolicyAction, Target, TargetRequirement,
    TriggerCondition, Zone,
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
    assert_eq!(ability.targets, [TargetRequirement::Player]);
    assert_eq!(
        ability.effects,
        [Effect::MillTargetPlayerFromSourceDamage],
        "the source-damage amount must be materialized by the deferred trigger scheduler"
    );
}

#[test]
fn belltower_damage_waits_for_controller_target_then_mills_captured_amount() {
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

    let choice = game
        .view_for_player(PlayerId(0))
        .expect("Sphinx controller view")
        .triggered_ability_target_choice
        .expect("recipient-damage trigger requires controller target choice");
    assert_eq!(choice.source, sphinx);
    assert_eq!(choice.ability, "damage-target-player-mill-that-many");
    assert_eq!(
        choice.target_options,
        [vec![
            Target::Player(PlayerId(0)),
            Target::Player(PlayerId(1))
        ]]
    );
    assert!(
        game.stack.is_empty(),
        "no target-bearing trigger reaches the stack early"
    );
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == sphinx && *ability == "damage-target-player-mill-that-many"
    )));

    game.submit_policy_move(
        PlayerId(0),
        "test.belltower-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: sphinx,
            ability: "damage-target-player-mill-that-many",
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("controller chooses the opponent to mill");
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");

    println!(
        "Belltower scheduler trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(milled), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == sphinx && *ability == "damage-target-player-mill-that-many"
    )));
    game.validate_invariants()
        .expect("Belltower scheduler trace preserves invariants");
}
