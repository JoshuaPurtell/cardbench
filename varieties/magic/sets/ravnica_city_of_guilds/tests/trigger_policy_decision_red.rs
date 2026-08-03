//! Red regression: a target-bearing trigger must wait for its controller's
//! explicit policy choice instead of silently selecting the first legal target.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, PlayerId, PolicyAction, Step, Target, Zone,
};
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

#[test]
#[allow(clippy::too_many_lines)] // The complete priority-boundary regression is intentionally linear.
fn optional_trigger_payment_does_not_auto_pay_at_resolution() {
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
    let meditation = game
        .put_on_battlefield(PlayerId(0), "RAV-SEARING-MEDITATION")
        .expect("meditation enters");
    let helix = game
        .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("helix enters hand");
    let mountains = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("mountain enters")
        })
        .collect::<Vec<_>>();
    let plains = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("plains enters")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("active pass");
        game.pass_priority(PlayerId(1)).expect("opponent pass");
    }
    for land in mountains {
        game.activate_mana_ability(PlayerId(0), land, Color::Red)
            .expect("red mana");
    }
    for land in plains {
        game.activate_mana_ability(PlayerId(0), land, Color::White)
            .expect("white mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: helix,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("helix casts");
    game.pass_priority(PlayerId(0)).expect("spell pass");
    game.pass_priority(PlayerId(1)).expect("helix resolves");
    game.pass_priority(PlayerId(0)).expect("trigger pass");
    game.pass_priority(PlayerId(1))
        .expect("resolution reaches payment choice");

    println!("event log after trigger passes: {:#?}", game.event_log);
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::AbilityManaPaid {
                source,
                ability: "life-gain-deal-two",
                ..
            } if *source == meditation
        )),
        "the engine auto-paid an optional triggered cost"
    );
    assert_eq!(
        game.stack.len(),
        1,
        "the trigger must stay suspended until the policy decides"
    );
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .optional_triggered_ability_choice
        .expect("optional payment choice is visible");
    assert_eq!(choice.source, meditation);
    assert_eq!(choice.ability, "life-gain-deal-two");
    assert!(choice.can_pay);
    assert!(
        choice
            .conditional_targets
            .contains(&Target::Player(PlayerId(1)))
    );
    game.submit_policy_move(
        PlayerId(0),
        "test.pay-optional-trigger.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: choice.decision,
            source: meditation,
            ability: "life-gain-deal-two",
            pay: true,
            target: Some(Target::Player(PlayerId(1))),
        },
    )
    .expect("controller pays and chooses the opponent");
    assert_eq!(game.stack.len(), 0);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityManaPaid {
            source,
            ability: "life-gain-deal-two",
            ..
        } if *source == meditation
    )));
    game.validate_invariants()
        .expect("optional payment transition is valid");
}
