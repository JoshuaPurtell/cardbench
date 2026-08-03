//! Full-fidelity contracts for Spawnbroker's dependent two-target ETB.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, Layer, PlayerId,
    PolicyAction, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

const SPAWNBROKER: &str = "RAV-SPAWNBROKER";
const RECRUIT: &str = "RAV-BOROS-RECRUIT";
const LARGE_CREATURE: &str = "RAV-WATCHWOLF";
const LAST_GASP: &str = "RAV-LAST-GASP";
const EXCHANGE_ABILITY: &str = "etb-may-exchange-control-creatures";

fn game() -> Game {
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

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != Step::PrecombatMain {
        pass_pair(game);
    }
}

struct SpawnbrokerFixture {
    game: Game,
    broker: cardbench_magic_engine::ObjectId,
    recruit: cardbench_magic_engine::ObjectId,
    large_opponent: cardbench_magic_engine::ObjectId,
    opponent_swamp: cardbench_magic_engine::ObjectId,
    last_gasp: cardbench_magic_engine::ObjectId,
}

fn fixture() -> SpawnbrokerFixture {
    let mut game = game();
    let broker = game
        .add_card(PlayerId(0), SPAWNBROKER, Zone::Hand)
        .expect("Spawnbroker starts in hand");
    let recruit = game
        .put_on_battlefield(PlayerId(1), RECRUIT)
        .expect("one-power opponent creature enters");
    let large_opponent = game
        .put_on_battlefield(PlayerId(1), LARGE_CREATURE)
        .expect("large opponent creature enters");
    let islands = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("Island enters")
        })
        .collect::<Vec<_>>();
    let opponent_swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("opponent Swamp enters");
    let last_gasp = game
        .add_card(PlayerId(1), LAST_GASP, Zone::Hand)
        .expect("Last Gasp starts in opponent hand");
    for player in [PlayerId(0), PlayerId(1)] {
        game.add_card(player, RECRUIT, Zone::Library)
            .expect("library setup card enters");
    }
    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("Spawnbroker mana is available");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: broker,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Spawnbroker cast succeeds");
    pass_pair(&mut game);
    SpawnbrokerFixture {
        game,
        broker,
        recruit,
        large_opponent,
        opponent_swamp,
        last_gasp,
    }
}

fn submit_target_pair(
    game: &mut Game,
    broker: cardbench_magic_engine::ObjectId,
    opponent: cardbench_magic_engine::ObjectId,
) {
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("targeted ETB opens a generic decision");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityTargets);
    assert_eq!((decision.min_selections, decision.max_selections), (2, 2));
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Targets(vec![Target::Permanent(broker), Target::Permanent(opponent)]),
    )
    .expect("legal exchange target pair is submitted");
}

#[test]
fn spawnbroker_exchanges_two_targets_and_the_other_half_survives_source_departure() {
    let SpawnbrokerFixture {
        mut game,
        broker,
        recruit,
        opponent_swamp,
        last_gasp,
        ..
    } = fixture();
    submit_target_pair(&mut game, broker, recruit);
    assert_eq!(
        game.stack.last().expect("ETB is on stack").targets,
        vec![Target::Permanent(broker), Target::Permanent(recruit)]
    );
    pass_pair(&mut game);
    game.submit_policy_move(
        PlayerId(0),
        "spawnbroker-accept.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: broker,
            ability: EXCHANGE_ABILITY,
            pay: true,
            target: None,
        },
    )
    .expect("controller accepts exchange");

    assert_eq!(game.controller_of(broker), Ok(PlayerId(1)));
    assert_eq!(game.controller_of(recruit), Ok(PlayerId(0)));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, layer: Layer::Control }
            if *source == broker && *target == broker
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { source, target, from, to }
            if *source == broker && *target == broker && *from == PlayerId(0) && *to == PlayerId(1)
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { source, target, from, to }
            if *source == recruit && *target == recruit && *from == PlayerId(1) && *to == PlayerId(0)
    )));

    game.pass_priority(PlayerId(0))
        .expect("original controller passes");
    game.activate_mana_ability(PlayerId(1), opponent_swamp, Color::Black)
        .expect("opponent has black mana");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: last_gasp,
            targets: vec![Target::Permanent(broker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent can remove its newly controlled broker");
    pass_pair(&mut game);
    println!(
        "Spawnbroker exchange trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(broker), Some(Zone::Graveyard));
    assert_eq!(
        game.controller_of(recruit),
        Ok(PlayerId(0)),
        "the recruit side of a completed exchange is not tied to broker survival"
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&SPAWNBROKER));
    game.validate_invariants()
        .expect("exchange and source-departure trace is invariant-valid");
}

#[test]
fn spawnbroker_rejects_an_overpowered_second_target_without_mutation_then_may_decline() {
    let SpawnbrokerFixture {
        mut game,
        broker,
        recruit,
        large_opponent,
        ..
    } = fixture();
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("targeted ETB opens its pair decision");
    let events_before = game.event_log.clone();
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Targets(vec![
                Target::Permanent(broker),
                Target::Permanent(large_opponent),
            ]),
        )
        .is_err(),
        "the second target exceeds the first target's current power"
    );
    assert_eq!(game.event_log, events_before);
    assert_eq!(game.stack.len(), 0);
    assert_eq!(
        game.view_for_player(PlayerId(0))
            .expect("controller view after rejection")
            .pending_decision
            .map(|pending| pending.id),
        Some(decision.id)
    );

    submit_target_pair(&mut game, broker, recruit);
    pass_pair(&mut game);
    game.submit_policy_move(
        PlayerId(0),
        "spawnbroker-decline.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: broker,
            ability: EXCHANGE_ABILITY,
            pay: false,
            target: None,
        },
    )
    .expect("controller may decline the completed target pair");
    assert_eq!(game.controller_of(broker), Ok(PlayerId(0)));
    assert_eq!(game.controller_of(recruit), Ok(PlayerId(1)));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::ControllerChanged { .. })),
        "declining produces no control mutation"
    );
    println!(
        "Spawnbroker decline trace: {:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("declined target-pair trigger remains invariant-valid");
}

#[test]
fn spawnbroker_never_partially_exchanges_when_a_selected_target_leaves_before_resolution() {
    let SpawnbrokerFixture {
        mut game,
        broker,
        recruit,
        opponent_swamp,
        last_gasp,
        ..
    } = fixture();
    submit_target_pair(&mut game, broker, recruit);

    // Target selection is complete, but the trigger has not begun resolving:
    // the opponent gets the ordinary priority window to remove the recruit.
    game.pass_priority(PlayerId(0))
        .expect("controller yields priority after target selection");
    game.activate_mana_ability(PlayerId(1), opponent_swamp, Color::Black)
        .expect("opponent has black mana for the response");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: last_gasp,
            targets: vec![Target::Permanent(recruit)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent removes the second target in response");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(recruit), Some(Zone::Graveyard));

    // The original ETB now reaches its controller's may decision. Accepting
    // it must not exchange only the still-legal first target.
    pass_pair(&mut game);
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .optional_triggered_ability_choice
        .expect("the optional exchange remains a policy choice");
    assert_eq!(choice.source, broker);
    assert_eq!(choice.ability, EXCHANGE_ABILITY);
    game.submit_policy_move(
        PlayerId(0),
        "spawnbroker-illegal-pair.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: choice.decision,
            source: broker,
            ability: EXCHANGE_ABILITY,
            pay: true,
            target: None,
        },
    )
    .expect("resolving an illegal pair is a legal no-op");

    assert_eq!(game.controller_of(broker), Ok(PlayerId(0)));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::ControllerChanged { source, .. } if *source == broker || *source == recruit
        )),
        "the paired instruction cannot install only one half of an exchange"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TargetInstructionSkipped { target, .. }
            if *target == Target::Permanent(recruit)
    )));
    println!(
        "Spawnbroker illegal-pair trace: {:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("lost target cannot leave a partial control exchange behind");
}
