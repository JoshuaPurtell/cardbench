//! Regression that the Augermage trigger observes the source's own death only.

use cardbench_magic_engine::{ContinuousChange, Duration, Game, GameEvent, PlayerId, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn sadistic_augermage_triggers_on_its_own_death_not_another_creature_death() {
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
        .expect("Sadistic Augermage definition exists");
    game.set_entered_turn_for_setup(augermage, 0)
        .expect("Augermage predates measured turn");
    let unrelated = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-GRAVE-TROLL")
        .expect("zero-toughness comparison creature enters");

    game.begin_game().expect("fixture starts");
    assert_eq!(game.zone_of(unrelated), Some(Zone::Graveyard));
    assert_eq!(
        game.stack.len(),
        0,
        "another creature's death does not trigger it"
    );

    game.add_continuous_effect(
        augermage,
        augermage,
        ContinuousChange::ModifyPowerToughness {
            power: 0,
            toughness: -1,
        },
        Duration::EndOfTurn(1),
    )
    .expect("the setup modifier makes the Augermage die to an SBA");
    assert_eq!(game.zone_of(augermage), Some(Zone::Graveyard));
    assert_eq!(
        game.stack.len(),
        1,
        "the source's own death queues its trigger"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == augermage
                && *ability == "source-dies-each-player-puts-card-on-library-top"
    )));
    game.validate_invariants()
        .expect("the source-death trigger boundary is valid");
}
