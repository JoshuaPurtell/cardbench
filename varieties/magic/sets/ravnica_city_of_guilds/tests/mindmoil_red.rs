//! Red-to-green contract for Mindmoil's cast-trigger hand recycle.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionSelection, DecisionVisibility, Game, GameEvent, ManaCost,
    PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_triggers() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger fixture builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_step(game: &mut Game, step: Step) {
    for _ in 0..16 {
        if game.step == step {
            return;
        }
        if game.step == Step::DeclareAttackers {
            game.declare_attackers(game.active_player, &[])
                .expect("fixture declares no attackers while advancing");
        }
        pass_pair(game);
    }
    panic!("fixture did not reach {step:?}");
}

#[test]
fn mindmoil_requires_exact_cast_trigger_hand_bottom_draw_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MINDMOIL")
        .expect("Mindmoil definition exists");

    assert_eq!(
        executable_definition_id_for_collector(135),
        Ok("RAV-MINDMOIL")
    );
    assert_eq!(definition.name, "Mindmoil");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Red])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"controller-casts-spell-private-hand-bottom-draw-same-count")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.ability.id == "controller-casts-spell-hand-bottom-draw-same-count"
    }));
}

#[test]
#[allow(clippy::too_many_lines)] // The private hand reorder/draw transcript is intentionally end-to-end.
fn mindmoil_privately_orders_exact_hand_then_draws_the_same_count() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game_with_rav_triggers();
    let mindmoil = game
        .put_on_battlefield(controller, "RAV-MINDMOIL")
        .expect("Mindmoil setup");
    let spell = game
        .add_card(controller, "RAV-LAST-GASP", Zone::Hand)
        .expect("spell setup");
    let first_recycled = game
        .add_card(controller, "RAV-WATCHWOLF", Zone::Hand)
        .expect("first recycle setup");
    let second_recycled = game
        .add_card(controller, "RAV-GLASS-GOLEM", Zone::Hand)
        .expect("second recycle setup");
    let first_draw = game
        .add_card(controller, "RAV-BIRDS-OF-PARADISE", Zone::Library)
        .expect("first draw setup");
    let second_draw = game
        .add_card(controller, "RAV-SNAPPING-DRAKE", Zone::Library)
        .expect("second draw setup");
    let target = game
        .put_on_battlefield(opponent, "RAV-WATCHWOLF")
        .expect("spell target setup");
    let swamp = game
        .put_on_battlefield(controller, "RAV-SWAMP")
        .expect("spell payment setup");
    let printed_cost_generic = game.put_on_battlefield(controller, "RAV-SWAMP").unwrap();

    game.begin_game().expect("game begins");
    advance_to_step(&mut game, Step::PrecombatMain);
    game.activate_mana_ability(controller, swamp, Color::Black)
        .expect("Swamp pays the cast");
    game.activate_mana_ability(controller, printed_cost_generic, Color::Black).unwrap();
    game.cast_spell(
        controller,
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell cast queues Mindmoil trigger");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == mindmoil
                && *ability == "controller-casts-spell-hand-bottom-draw-same-count"
    )));

    pass_pair(&mut game);
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("Mindmoil opens a private hand-order decision");
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!((decision.min_selections, decision.max_selections), (2, 2));
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![first_recycled, second_recycled]
    );
    assert!(
        game.view_for_player(opponent)
            .expect("opponent view")
            .pending_decision
            .is_none()
    );
    assert!(
        game.submit_decision(
            controller,
            decision.id,
            DecisionSelection::Objects(vec![first_recycled]),
        )
        .is_err(),
        "a partial hand answer cannot commit any zone movement",
    );
    assert_eq!(game.zone_of(first_recycled), Some(Zone::Hand));
    assert_eq!(game.zone_of(second_recycled), Some(Zone::Hand));
    assert_eq!(
        game.view_for_player(controller)
            .expect("controller view after rejection")
            .pending_decision
            .expect("the exact snapshot remains pending after rejection")
            .id,
        decision.id
    );

    game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![second_recycled, first_recycled]),
    )
    .expect("controller submits exact private bottom-to-top order");
    assert_eq!(game.zone_of(first_recycled), Some(Zone::Library));
    assert_eq!(game.zone_of(second_recycled), Some(Zone::Library));
    assert_eq!(game.zone_of(first_draw), Some(Zone::Hand));
    assert_eq!(game.zone_of(second_draw), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::HandPutOnLibraryBottomThenDrawn {
            player,
            source,
            cards: 2,
            ..
        } if *player == controller && *source == mindmoil
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == mindmoil
                && *ability == "controller-casts-spell-hand-bottom-draw-same-count"
    )));

    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("private hand recycle preserves all state-machine invariants");
    eprintln!("mindmoil_trace={:?}", game.canonical_event_log());
}

#[test]
fn mindmoil_observes_a_controller_creature_spell_exactly_once() {
    let controller = PlayerId(0);
    let mut game = game_with_rav_triggers();
    let mindmoil = game
        .put_on_battlefield(controller, "RAV-MINDMOIL")
        .expect("Mindmoil setup");
    let creature_spell = game
        .add_card(controller, "RAV-WATCHWOLF", Zone::Hand)
        .expect("creature spell setup");
    let recycled = game
        .add_card(controller, "RAV-GLASS-GOLEM", Zone::Hand)
        .expect("recycle setup");
    let drawn = game
        .add_card(controller, "RAV-BIRDS-OF-PARADISE", Zone::Library)
        .expect("draw setup");
    let forest = game
        .put_on_battlefield(controller, "RAV-FOREST")
        .expect("green payment setup");
    let plains = game
        .put_on_battlefield(controller, "RAV-PLAINS")
        .expect("white payment setup");

    game.begin_game().expect("game begins");
    advance_to_step(&mut game, Step::PrecombatMain);
    game.activate_mana_ability(controller, forest, Color::Green)
        .expect("Forest pays creature spell");
    game.activate_mana_ability(controller, plains, Color::White)
        .expect("Plains pays creature spell");
    game.cast_spell(
        controller,
        CastRequest {
            card: creature_spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("creature cast queues one Mindmoil trigger");
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == mindmoil
                        && *ability == "controller-casts-spell-hand-bottom-draw-same-count"
            ))
            .count(),
        1,
        "a creature spell must queue the one generic cast trigger, not zero or two",
    );
    pass_pair(&mut game);
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("one-card private hand snapshot opens");
    game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![recycled]),
    )
    .expect("one-card snapshot commits");
    assert_eq!(game.zone_of(recycled), Some(Zone::Library));
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    pass_pair(&mut game);
    assert_eq!(game.zone_of(creature_spell), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("creature-cast hand recycle preserves engine invariants");
}
