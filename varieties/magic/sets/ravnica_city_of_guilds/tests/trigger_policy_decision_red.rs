//! Red regression: a target-bearing trigger must wait for its controller's
//! explicit policy choice instead of silently selecting the first legal target.

use cardbench_magic_engine::{Game, GameEvent, PlayerId, Step};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn frenzied_goblin_does_not_auto_select_a_trigger_target() {
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
    let goblin = game
        .put_on_battlefield(PlayerId(0), "RAV-FRENZIED-GOBLIN")
        .expect("goblin enters");
    let first = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("first target enters");
    let second = game
        .put_on_battlefield(PlayerId(1), "RAV-GLASS-GOLEM")
        .expect("second target enters");
    for creature in [goblin, first, second] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("old fixture entry");
    }
    game.begin_game().expect("game starts");
    while game.step != Step::DeclareAttackers {
        let player = game.priority;
        game.pass_priority(player).expect("advance to attackers");
    }

    game.declare_attackers(PlayerId(0), &[goblin])
        .expect("goblin attacks");
    println!("event log before trigger choice: {:#?}", game.event_log);

    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                ability: "attack-cannot-block",
                ..
            } if *source == goblin
        )),
        "the engine auto-selected one of two legal trigger targets"
    );
    assert_eq!(game.stack.len(), 0, "no trigger may stack before the choice");
}
