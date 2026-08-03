//! Red discovery contract for Sadistic Augermage's optional dies trigger.

use cardbench_magic_engine::{Game, GameEvent, PlayerId, PolicyAction, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn sadistic_augermage_controller_can_decline_the_dies_trigger() {
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
    let augermage = game
        .put_on_battlefield(PlayerId(0), "RAV-SADISTIC-AUGERMAGE")
        .expect("Augermage enters");
    game.set_entered_turn_for_setup(augermage, 0)
        .expect("Augermage predates measured turn");
    let victim = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-GRAVE-TROLL")
        .expect("zero-toughness victim enters");
    game.set_entered_turn_for_setup(victim, 0)
        .expect("victim predates measured turn");
    let controller_hand = game
        .add_card(PlayerId(0), "RAV-SWAMP", Zone::Hand)
        .expect("controller hand card enters");
    let opponent_hand = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Hand)
        .expect("opponent hand card enters");

    game.begin_game().expect("fixture starts");
    assert_eq!(game.stack.len(), 1, "death trigger is queued");
    game.pass_priority(PlayerId(0)).expect("active passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and trigger reaches its may choice");

    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .optional_triggered_ability_choice
        .expect("controller receives the public may-trigger choice");
    assert_eq!(choice.source, augermage);
    assert_eq!(choice.ability, "another-creature-dies-each-player-discards");
    assert!(
        choice.can_pay,
        "zero-cost may trigger can always be accepted"
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .optional_triggered_ability_choice
            .is_none(),
        "only the trigger controller may accept or decline"
    );

    game.submit_policy_move(
        PlayerId(0),
        "test.sadistic-augermage-decline.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            source: augermage,
            ability: "another-creature-dies-each-player-discards",
            pay: false,
            target: None,
        },
    )
    .expect("controller can decline Sadistic Augermage's may trigger");

    eprintln!(
        "Sadistic Augermage declined-may trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(controller_hand), Some(Zone::Hand));
    assert_eq!(game.zone_of(opponent_hand), Some(Zone::Hand));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::CardDiscarded { card, .. }
                if *card == controller_hand || *card == opponent_hand
        )),
        "a declined trigger cannot open or complete either discard choice"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == augermage && *ability == "another-creature-dies-each-player-discards"
    )));
    game.validate_invariants()
        .expect("declined trigger preserves the state machine");
}
