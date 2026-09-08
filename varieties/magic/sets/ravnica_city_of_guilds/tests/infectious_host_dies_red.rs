//! Red coverage probe for Infectious Host's dies trigger.

use cardbench_magic_engine::{
    CardType, Color, ContinuousChange, DecisionSelection, Duration, Game, GameEvent, ManaCost,
    PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn infectious_host_exposes_its_targeted_dies_life_loss_slice() {
    let host = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INFECTIOUS-HOST")
        .expect("Infectious Host definition exists");
    assert_eq!(host.mana_cost, ManaCost::with_colors(2, [Color::Black]));
    assert_eq!(host.card_types, [CardType::Creature].into_iter().collect());
    assert_eq!((host.power, host.toughness), (Some(1), Some(1)));
    assert!(
        host.supported_rules
            .contains(&"dies-target-player-life-loss")
    );
    assert!(
        host.supported_rules.contains(&"full-rules-fidelity"),
        "Infectious Host has only a typed, policy-selected dies trigger; it must not retain the old deterministic compatibility classification"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&host.id),
        "the complete policy-selected dies trigger should promote Infectious Host"
    );
}

#[test]
fn infectious_host_dies_trigger_targets_one_player_and_records_life_loss() {
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
    let host = game
        .add_card(PlayerId(0), "RAV-INFECTIOUS-HOST", Zone::Battlefield)
        .expect("Host enters battlefield");
    game.begin_game().expect("game begins");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("advance to precombat main");
    }
    game.add_continuous_effect(
        host,
        host,
        ContinuousChange::ModifyPowerToughness {
            power: -1,
            toughness: -1,
        },
        Duration::EndOfTurn(1),
    )
    .expect("SBA destroys the Host");
    assert_eq!(game.zone_of(host), Some(Zone::Graveyard));
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("Host controller receives the trigger target decision")
        .pending_decision
        .expect("target-bearing dies trigger opens a policy decision");
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Targets(vec![Target::Player(PlayerId(1))]),
    )
    .expect("Host controller chooses the opposing player");
    assert_eq!(
        game.stack.last().map(|object| object.targets.clone()),
        Some(vec![Target::Player(PlayerId(1))])
    );
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("dies trigger resolves");

    println!("Infectious Host trace: {:?}", game.canonical_event_log());
    assert_eq!(game.player(PlayerId(1)).expect("opponent exists").life, 18);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeLost { source, player, amount }
            if *source == host && *player == PlayerId(1) && *amount == 2
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == host && *ability == "dies-target-player-life-loss"
    )));
    game.validate_invariants()
        .expect("policy-targeted dies trigger preserves state-machine invariants");
}
