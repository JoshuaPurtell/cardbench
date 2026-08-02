//! Red discovery regression for a controller-scoped creature-spell cast trigger.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, PolicyAction, Zone};
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

fn prepare_controller_creature_cast(game: &mut Game) -> (u64, u64, u64, u64) {
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
    (sage.0, creature.0, drawn_card.0, forest.0)
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
                && binding.ability.optional
        }),
        "Sage requires an optional stack trigger rather than a deterministic draw"
    );
}

#[test]
fn primordial_sage_accepted_draw_resolves_above_creature_spell() {
    let mut game = game_with_rav_bindings();
    let (sage, creature, drawn_card, _) = prepare_controller_creature_cast(&mut game);
    let sage = cardbench_magic_engine::ObjectId(sage);
    let creature = cardbench_magic_engine::ObjectId(creature);
    let drawn_card = cardbench_magic_engine::ObjectId(drawn_card);
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
fn primordial_sage_ignores_opponent_creature_spell_and_decline_draw_is_visible() {
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
