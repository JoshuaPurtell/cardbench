//! Complete event-log contract for Hunted Phantasm's targeted entry trigger.

use cardbench_magic_engine::{
    CastRequest, Color, CreatureSubtype, DecisionKind, DecisionVisibility, Game, GameEvent,
    Keyword, PlayerId, PolicyAction, RulesError, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings, run_all_scenarios,
};

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != Step::PrecombatMain {
        if game.step == Step::Draw
            && game
                .view_for_player(game.active_player)
                .expect("active-player view")
                .draw_replacement_pending
        {
            let player = game.active_player;
            game.resolve_pending_draw(player, None)
                .expect("multiplayer first draw resolves before priority");
            continue;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority pass advances the turn");
    }
}

fn resolve_top(game: &mut Game) {
    let stack_depth = game.stack.len();
    while game.stack.len() == stack_depth {
        let player = game.priority;
        game.pass_priority(player)
            .expect("every living player passes the stack item");
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The three-player authority trace is intentionally linear.
fn hunted_phantasm_controller_selects_the_second_opponent_for_exact_goblins() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-PHANTASM")
        .expect("Hunted Phantasm definition exists");
    assert!(definition.keywords.contains(&Keyword::Unblockable));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));

    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        3,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let phantasm = game
        .add_card(PlayerId(0), "RAV-HUNTED-PHANTASM", Zone::Hand)
        .expect("Hunted Phantasm is executable");
    game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("multiplayer first-draw fixture card enters the library");
    let islands = [
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Battlefield)
            .expect("first Island enters"),
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Battlefield)
            .expect("second Island enters"),
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Battlefield)
            .expect("third Island enters"),
    ];

    game.begin_game().expect("fixture starts game");
    advance_to_precombat_main(&mut game);
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("Island produces blue mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: phantasm,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Hunted Phantasm casts without a spell target");
    resolve_top(&mut game);

    let controller_view = game
        .view_for_player(PlayerId(0))
        .expect("trigger controller sees the decision");
    let decision = controller_view
        .pending_decision
        .expect("targeted ETB waits outside the stack for its controller");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityTargets);
    assert_eq!(
        decision.target_candidates,
        [Target::Player(PlayerId(1)), Target::Player(PlayerId(2))]
    );
    let target_choice = controller_view
        .triggered_ability_target_choice
        .expect("controller sees the compatible target-choice projection");
    assert_eq!(target_choice.source, phantasm);
    assert_eq!(target_choice.ability, "etb-opponent-goblins");
    assert_eq!(
        target_choice.target_options,
        vec![vec![
            Target::Player(PlayerId(1)),
            Target::Player(PlayerId(2))
        ]]
    );
    for opponent in [PlayerId(1), PlayerId(2)] {
        let view = game
            .view_for_player(opponent)
            .expect("opponent view remains available");
        assert!(matches!(
            view.pending_decision,
            Some(ref public_decision)
                if public_decision.kind == DecisionKind::TriggeredAbilityTargets
                    && public_decision.visibility == DecisionVisibility::Public
                    && public_decision.target_candidates == [
                        Target::Player(PlayerId(1)),
                        Target::Player(PlayerId(2)),
                    ]
        ));
        assert!(view.triggered_ability_target_choice.is_none());
    }

    let pre_unauthorized_events = game.canonical_event_log();
    let unauthorized = game.submit_policy_move(
        PlayerId(1),
        "test.hunted-phantasm-unauthorized-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: phantasm,
            ability: "etb-opponent-goblins",
            targets: vec![Target::Player(PlayerId(2))],
        },
    );
    assert!(matches!(unauthorized, Err(RulesError::IllegalAction(_))));
    assert_eq!(
        game.canonical_event_log(),
        pre_unauthorized_events,
        "a non-controller cannot manufacture an accepted target-selection receipt"
    );

    game.submit_policy_move(
        PlayerId(0),
        "test.hunted-phantasm-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: phantasm,
            ability: "etb-opponent-goblins",
            targets: vec![Target::Player(PlayerId(2))],
        },
    )
    .expect("controller selects the second legal opponent");
    assert_eq!(
        game.stack.last().map(|object| object.targets.clone()),
        Some(vec![Target::Player(PlayerId(2))]),
        "the ETB trigger preserves the policy-selected opponent"
    );
    resolve_top(&mut game);

    println!("Hunted Phantasm trace: {:#?}", game.canonical_event_log());
    let tokens = &game
        .player(PlayerId(2))
        .expect("chosen opponent exists")
        .battlefield;
    assert_eq!(tokens.len(), 5, "exactly five Goblins are created");
    assert!(
        game.player(PlayerId(1))
            .expect("unselected opponent exists")
            .battlefield
            .is_empty(),
        "the first opponent receives no tokens"
    );
    for token in tokens {
        let object = game.object(*token).expect("token object exists");
        assert_eq!(object.owner, PlayerId(2));
        assert_eq!(game.controller_of(*token), Ok(PlayerId(2)));
        let characteristics = game.characteristics(*token).expect("token characteristics");
        assert_eq!(characteristics.colors, [Color::Red].into_iter().collect());
        assert_eq!(characteristics.power, Some(1));
        assert_eq!(characteristics.toughness, Some(1));
        assert!(
            characteristics
                .creature_subtypes
                .contains(&CreatureSubtype::Goblin)
        );
    }
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::TokenCreated { player, .. } if *player == PlayerId(2)))
            .count(),
        5
    );
    assert!(
        !game.event_log.iter().any(
            |event| matches!(event, GameEvent::TokenCreated { player, .. } if *player == PlayerId(1))
        ),
        "the event log must not claim an unselected recipient"
    );
    let chosen = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::DecisionCompleted {
                    player,
                    kind: DecisionKind::TriggeredAbilityTargets,
                    ..
                } if *player == PlayerId(0)
            )
        })
        .expect("controller target decision receipt");
    let stacked = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == phantasm && *ability == "etb-opponent-goblins"
            )
        })
        .expect("targeted trigger stack receipt");
    let created = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::TokenCreated { player, .. } if *player == PlayerId(2)))
        .expect("chosen opponent token receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == phantasm && *ability == "etb-opponent-goblins"
            )
        })
        .expect("targeted trigger terminal receipt");
    assert!(chosen < stacked && stacked < created && created < resolved);
    game.validate_invariants()
        .expect("Hunted Phantasm ETB preserves invariants");
}

#[test]
fn hunted_phantasm_public_scenario_records_the_target_decision_and_goblins() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_hunted_phantasm_policy_target_goblins")
        .expect("Hunted Phantasm public scenario exists");
    println!("Hunted Phantasm shown trace: {:#?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:1dcc9deafeb1b9a5");
    for marker in [
        "DecisionOpened",
        "DecisionCompleted",
        "TriggeredAbilityStacked",
        "TokenCreated",
        "AbilityResolved",
    ] {
        assert!(
            trace.event_log.iter().any(|event| event.contains(marker)),
            "shown trace lacks {marker}"
        );
    }
    assert_eq!(
        trace
            .event_log
            .iter()
            .filter(|event| event.contains("TokenCreated"))
            .count(),
        5
    );
}
