//! Stack and event-log contract for Sadistic Augermage's death trigger.

use cardbench_magic_engine::{Game, GameEvent, PlayerId, PolicyAction, Step, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

#[test]
fn another_creature_dying_stacks_then_resolves_all_player_discards() {
    let mut game = game_with_rav_bindings();
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
    let p0_discard = game
        .add_card(PlayerId(0), "RAV-SWAMP", Zone::Hand)
        .expect("controller hand card enters");
    let p0_keep = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("controller second hand card enters");
    let p1_discard = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Hand)
        .expect("opponent hand card enters");

    game.begin_game().expect("fixture starts");
    assert_eq!(game.step, Step::Upkeep);
    assert_eq!(game.stack.len(), 1, "death trigger is queued");
    game.pass_priority(PlayerId(0)).expect("active passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    let controller_choice = game
        .view_for_player(PlayerId(0))
        .expect("Augermage controller view")
        .triggered_ability_effect_object_choice
        .expect("controller chooses its discard");
    assert_eq!(controller_choice.candidates.len(), 2);
    game.submit_policy_move(
        PlayerId(0),
        "test.augermage-discard-p0.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            source: augermage,
            ability: "another-creature-dies-each-player-discards",
            selected: Some(p0_discard),
        },
    )
    .expect("controller selects its discard");
    let opponent_choice = game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .triggered_ability_effect_object_choice
        .expect("opponent chooses its discard");
    assert_eq!(opponent_choice.candidates.len(), 1);
    game.submit_policy_move(
        PlayerId(1),
        "test.augermage-discard-p1.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            source: augermage,
            ability: "another-creature-dies-each-player-discards",
            selected: Some(p1_discard),
        },
    )
    .expect("opponent selects its discard");

    println!(
        "Sadistic Augermage trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(victim), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(p0_discard), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(p1_discard), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(p0_keep), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::StateBasedAction { card, .. } if *card == victim)
    }));
    for (player, card) in [(PlayerId(0), p0_discard), (PlayerId(1), p1_discard)] {
        assert!(game.event_log.iter().any(|event| {
            matches!(event, GameEvent::CardDiscarded { player: event_player, card: event_card } if *event_player == player && *event_card == card)
        }));
    }
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::AbilityResolved {
                source,
                ability: "another-creature-dies-each-player-discards",
                ..
            } if *source == augermage
        )
    }));
    game.validate_invariants()
        .expect("discard trigger preserves invariants");
}
