//! Event-log contracts for Vedalken Dismisser's targeted entry trigger.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
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

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != Step::PrecombatMain {
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first response pass");
    let second = game.priority;
    game.pass_priority(second).expect("second response pass");
}

fn cast_dismisser_and_choose_target(
    game: &mut Game,
    dismisser: cardbench_magic_engine::ObjectId,
    target: cardbench_magic_engine::ObjectId,
    islands: &[cardbench_magic_engine::ObjectId],
) {
    for island in islands {
        game.activate_mana_ability(PlayerId(0), *island, Color::Blue)
            .expect("Dismisser mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: dismisser,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast Vedalken Dismisser");
    resolve_top(game);

    let controller_choice = game
        .view_for_player(PlayerId(0))
        .expect("Dismisser controller view")
        .triggered_ability_target_choice
        .expect("entry trigger waits for a policy target");
    assert_eq!(
        controller_choice.ability,
        "etb-target-creature-owner-library-top"
    );
    assert!(
        controller_choice
            .target_options
            .first()
            .is_some_and(|options| options.contains(&Target::Permanent(target))),
        "the exact live creature must be offered to the controller"
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .triggered_ability_target_choice
            .is_none(),
        "only the ability controller may select this trigger target"
    );

    game.submit_policy_move(
        PlayerId(0),
        "test.vedalken-dismisser-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: dismisser,
            ability: "etb-target-creature-owner-library-top",
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("policy selects the target creature");
}

#[test]
fn dismisser_target_choice_places_the_exact_creature_on_its_owners_library_top() {
    let mut game = game_with_rav_bindings();
    let dismisser = game
        .add_card(PlayerId(0), "RAV-VEDALKEN-DISMISSER", Zone::Hand)
        .expect("Dismisser setup");
    let library_bottom = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
        .expect("opponent library bottom");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature setup");
    let islands = (0..6)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("Dismisser mana source")
        })
        .collect::<Vec<_>>();

    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    cast_dismisser_and_choose_target(&mut game, dismisser, target, &islands);

    let trigger = game.stack.last().expect("targeted entry trigger stacks");
    assert_eq!(trigger.card, dismisser);
    assert_eq!(trigger.targets, vec![Target::Permanent(target)]);
    assert_eq!(
        trigger.target_incarnations,
        vec![Some(
            game.object(target).expect("target exists").incarnation
        )],
        "the trigger must capture the targeted creature incarnation"
    );
    resolve_top(&mut game);

    println!(
        "vedalken_dismisser_top_library_event_log={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(target), Some(Zone::Library));
    assert_eq!(
        game.players[PlayerId(1).0].library,
        vec![library_bottom, target],
        "the targeted creature becomes its owner's draw-top card"
    );
    let trigger_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == dismisser && *ability == "etb-target-creature-owner-library-top"
            )
        })
        .expect("trigger stack receipt");
    let target_move_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardMoved { card, to: Zone::Library } if *card == target
            )
        })
        .expect("owner-library move receipt");
    let resolved_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == dismisser && *ability == "etb-target-creature-owner-library-top"
            )
        })
        .expect("trigger resolution receipt");
    assert!(
        trigger_index < target_move_index && target_move_index < resolved_index,
        "targeted trigger must stack before its ordinary owner-library move and terminal receipt"
    );
    game.validate_invariants()
        .expect("Dismisser target provenance and owner-zone transition remain valid");
}

#[test]
fn dismisser_entry_trigger_is_countered_when_a_response_removes_its_target() {
    let mut game = game_with_rav_bindings();
    let dismisser = game
        .add_card(PlayerId(0), "RAV-VEDALKEN-DISMISSER", Zone::Hand)
        .expect("Dismisser setup");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature setup");
    let peel = game
        .add_card(PlayerId(1), "RAV-PEEL-FROM-REALITY", Zone::Hand)
        .expect("response setup");
    let dismisser_islands = (0..6)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("Dismisser mana source")
        })
        .collect::<Vec<_>>();
    let response_islands = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-ISLAND")
                .expect("response mana source")
        })
        .collect::<Vec<_>>();

    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    cast_dismisser_and_choose_target(&mut game, dismisser, target, &dismisser_islands);

    game.pass_priority(PlayerId(0))
        .expect("Dismisser controller opens the response window");
    for island in response_islands {
        game.activate_mana_ability(PlayerId(1), island, Color::Blue)
            .expect("response mana");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: peel,
            targets: vec![Target::Permanent(target), Target::Permanent(dismisser)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("response returns the target before the trigger resolves");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Hand));

    resolve_top(&mut game);
    println!(
        "vedalken_dismisser_stale_target_event_log={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(target), Some(Zone::Hand));
    assert!(
        !game.players[PlayerId(1).0].library.contains(&target),
        "a departed target must not be moved by the old trigger identity"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityCounteredByRules { source, ability, .. }
            if *source == dismisser && *ability == "etb-target-creature-owner-library-top"
    )));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::AbilityResolved { source, ability, .. }
                if *source == dismisser && *ability == "etb-target-creature-owner-library-top"
        )),
        "an all-illegal targeted trigger gets the rules-counter terminal receipt"
    );
    game.validate_invariants()
        .expect("response-stale target trace remains state-machine valid");
}

#[test]
fn dismisser_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-VEDALKEN-DISMISSER"));
}
