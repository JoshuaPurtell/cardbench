//! Red regression: a target-bearing trigger must wait for its controller's
//! explicit policy choice instead of silently selecting the first legal target.

use cardbench_magic_engine::{Game, GameEvent, PlayerId, PolicyAction, Step, Target};
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
    assert_eq!(
        game.stack.len(),
        0,
        "no trigger may stack before the choice"
    );
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("target choice is visible");
    assert_eq!(choice.source, goblin);
    assert_eq!(choice.ability, "attack-cannot-block");
    assert!(choice.target_options[0].contains(&Target::Permanent(first)));
    assert!(choice.target_options[0].contains(&Target::Permanent(second)));
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .triggered_ability_target_choice
            .is_none(),
        "only the decision-maker receives the actionable choice"
    );

    game.submit_policy_move(
        PlayerId(0),
        "test.choose-trigger-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: goblin,
            ability: "attack-cannot-block",
            targets: vec![Target::Permanent(second)],
        },
    )
    .expect("controller selects the second target");
    assert_eq!(game.stack.len(), 1);
    assert!(matches!(
        game.event_log.iter().rev().nth(1),
        Some(GameEvent::TriggeredAbilityStacked {
            source,
            ability: "attack-cannot-block",
            ..
        }) if *source == goblin
    ));
    game.validate_invariants()
        .expect("choice transition is valid");
}
