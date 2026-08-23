//! Red discovery regression for a controller-scoped creature-spell cast trigger.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, ObjectId, PlayerId, PolicyAction, Target,
    TriggerCondition, Zone,
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

fn advance_to_first_main(game: &mut Game) {
    game.begin_game().expect("fixture begins game");
    for _ in 0..4 {
        game.pass_priority(game.priority)
            .expect("ordinary priority advances the first turn");
    }
}

fn prepare_controller_creature_cast(game: &mut Game) -> (ObjectId, ObjectId, ObjectId) {
    let sage = game
        .put_on_battlefield(PlayerId(0), "RAV-PRIMORDIAL-SAGE")
        .expect("Sage begins on battlefield");
    let creature = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("creature spell begins in hand");
    let drawn_card = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("draw target begins in library");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on battlefield");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains begins on battlefield");
    advance_to_first_main(game);
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest produces green mana");
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("Plains produces white mana");
    game.clear_event_log();
    (sage, creature, drawn_card)
}

#[test]
fn primordial_sage_requires_controller_creature_cast_optional_draw_contract() {
    let sage = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PRIMORDIAL-SAGE")
        .expect("Primordial Sage definition exists");
    assert!(
        sage.supported_rules
            .contains(&"controller-casts-creature-spell-may-draw"),
        "the controller-scoped creature-spell trigger must be explicit"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sage.id),
        "the complete trigger and policy-declared optional resolution make Sage full fidelity"
    );
    assert!(
        rav_triggered_ability_bindings().into_iter().any(|binding| {
            binding.card_definition == "RAV-PRIMORDIAL-SAGE"
                && binding.ability.id == "controller-creature-spell-cast-may-draw"
                && binding.ability.condition == TriggerCondition::CastsCreatureSpell
                && binding.ability.optional
        }),
        "Sage requires an optional stack trigger rather than a deterministic draw"
    );
}

#[test]
fn primordial_sage_accepted_draw_resolves_above_creature_spell() {
    let mut game = game_with_rav_bindings();
    let (sage, creature, drawn_card) = prepare_controller_creature_cast(&mut game);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: creature,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("controller casts a creature spell");
    assert_eq!(game.stack.len(), 2, "trigger must be above creature spell");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the trigger");
    game.pass_priority(PlayerId(1))
        .expect("trigger reaches its optional choice");
    game.submit_policy_move(
        PlayerId(0),
        "primordial-sage-test.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: sage,
            ability: "controller-creature-spell-cast-may-draw",
            pay: true,
            target: None,
        },
    )
    .expect("controller accepts the optional draw");
    assert_eq!(game.zone_of(drawn_card), Some(Zone::Hand));
    assert_eq!(game.stack.len(), 1, "creature spell remains below trigger");
    game.pass_priority(PlayerId(0))
        .expect("caster passes creature spell");
    game.pass_priority(PlayerId(1))
        .expect("creature spell resolves");
    println!(
        "primordial_sage_accept_event_log={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == sage && *ability == "controller-creature-spell-cast-may-draw"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == sage && *ability == "controller-creature-spell-cast-may-draw"
    )));
    game.validate_invariants()
        .expect("accepted Sage trigger preserves invariants");
}

#[test]
fn primordial_sage_ignores_opponent_creature_spell() {
    let mut game = game_with_rav_bindings();
    let sage = game
        .put_on_battlefield(PlayerId(1), "RAV-PRIMORDIAL-SAGE")
        .expect("opponent Sage begins on battlefield");
    let creature = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("player zero creature begins in hand");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on battlefield");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains begins on battlefield");
    advance_to_first_main(&mut game);
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest produces green mana");
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("Plains produces white mana");
    game.clear_event_log();
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: creature,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("player zero casts creature");
    assert_eq!(game.stack.len(), 1, "opponent Sage must not trigger");
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, .. } if *source == sage
    )));
    game.validate_invariants()
        .expect("opponent-scoped non-trigger preserves invariants");
}

#[test]
fn primordial_sage_decline_leaves_the_library_card_in_place() {
    let mut game = game_with_rav_bindings();
    let (sage, creature, drawn_card) = prepare_controller_creature_cast(&mut game);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: creature,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("controller casts a creature spell");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the trigger");
    game.pass_priority(PlayerId(1))
        .expect("trigger reaches its optional choice");
    game.submit_policy_move(
        PlayerId(0),
        "primordial-sage-test.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: sage,
            ability: "controller-creature-spell-cast-may-draw",
            pay: false,
            target: None,
        },
    )
    .expect("controller declines the optional draw");
    assert_eq!(game.zone_of(drawn_card), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == sage && *ability == "controller-creature-spell-cast-may-draw"
    )));
    game.pass_priority(PlayerId(0))
        .expect("caster passes creature spell");
    game.pass_priority(PlayerId(1))
        .expect("creature spell resolves");
    game.validate_invariants()
        .expect("declined Sage trigger preserves invariants");
}

#[test]
#[allow(clippy::too_many_lines)] // The source-departure trigger transcript is intentionally end-to-end.
fn primordial_sage_trigger_draws_after_its_source_leaves_before_resolution() {
    let mut game = game_with_rav_bindings();
    let putrefy = game
        .add_card(PlayerId(1), "RAV-PUTREFY", Zone::Hand)
        .expect("response begins in hand");
    let first_forest = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("first response Forest begins on battlefield");
    let second_forest = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("second response Forest begins on battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("response Swamp begins on battlefield");
    let (sage, creature, drawn_card) = prepare_controller_creature_cast(&mut game);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: creature,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("controller casts a creature spell");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the trigger to player one");
    for (land, color) in [
        (first_forest, Color::Green),
        (second_forest, Color::Green),
        (swamp, Color::Black),
    ] {
        game.activate_mana_ability(PlayerId(1), land, color)
            .expect("response land produces payment mana");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: putrefy,
            targets: vec![Target::Permanent(sage)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("player one responds by destroying the trigger source");
    let first = game.priority;
    game.pass_priority(first)
        .expect("response caster passes its spell");
    let second = game.priority;
    game.pass_priority(second).expect("response spell resolves");
    assert_eq!(game.zone_of(sage), Some(Zone::Graveyard));
    assert_eq!(
        game.stack.len(),
        2,
        "the source-less Sage trigger remains above the creature spell"
    );
    let first = game.priority;
    game.pass_priority(first)
        .expect("first pass reaches the source-less trigger");
    let second = game.priority;
    game.pass_priority(second)
        .expect("source-less trigger reaches the controller choice");
    game.submit_policy_move(
        PlayerId(0),
        "primordial-sage-test.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: sage,
            ability: "controller-creature-spell-cast-may-draw",
            pay: true,
            target: None,
        },
    )
    .expect("trigger remains independently resolvable");
    assert_eq!(game.zone_of(drawn_card), Some(Zone::Hand));
    let spell_cast = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellCast { card, .. } if *card == creature))
        .expect("creature cast receipt exists");
    let trigger_stacked = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == sage && *ability == "controller-creature-spell-cast-may-draw"
            )
        })
        .expect("Sage trigger receipt exists");
    assert!(spell_cast < trigger_stacked);
    println!(
        "primordial_sage_departed_source_event_log={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("source-less Sage trigger preserves invariants");
}
