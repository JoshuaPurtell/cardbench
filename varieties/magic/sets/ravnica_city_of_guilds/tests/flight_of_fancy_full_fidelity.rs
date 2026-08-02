//! Event-log contract for Flight of Fancy's full Aura and ETB behavior.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, Keyword, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
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

#[test]
fn flight_attaches_grants_flying_then_draws_two_through_its_own_stack_trigger() {
    let mut game = game_with_rav_bindings();
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("creature setup");
    let flight = game
        .add_card(PlayerId(0), "RAV-FLIGHT-OF-FANCY", Zone::Hand)
        .expect("Aura enters hand");
    let first_draw = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("first library card");
    let second_draw = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Library)
        .expect("second library card");
    game.grant_mana(PlayerId(0), Color::Blue, 4)
        .expect("fixture mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: flight,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Flight casts on creature");
    let first = game.priority;
    game.pass_priority(first).expect("caster passes Aura");
    let second = game.priority;
    game.pass_priority(second)
        .expect("Aura resolves and trigger stacks");

    assert_eq!(game.zone_of(flight), Some(Zone::Battlefield));
    assert_eq!(
        game.object(flight).expect("Aura exists").attached_to,
        Some(creature)
    );
    assert!(
        game.characteristics(creature)
            .expect("creature characteristics")
            .keywords
            .contains(&Keyword::Flying),
        "the attachment grants Flying before the trigger resolves"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == flight && *ability == "etb-draw-two"
    )));

    let first = game.priority;
    game.pass_priority(first)
        .expect("trigger controller passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("two-card draw trigger resolves");
    println!(
        "flight_of_fancy_event_log={:#?}",
        game.canonical_event_log()
    );

    for card in [first_draw, second_draw] {
        assert_eq!(game.zone_of(card), Some(Zone::Hand));
    }
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == flight && *ability == "etb-draw-two"
    )));
    game.validate_invariants()
        .expect("Flight event trace preserves invariants");
}
