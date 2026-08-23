//! Event-log contract for Fists of Ironwood's Aura and token trigger.

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
fn fists_attaches_grants_trample_then_creates_two_saprolings_through_its_own_trigger() {
    let mut game = game_with_rav_bindings();
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("creature setup");
    let fists = game
        .add_card(PlayerId(0), "RAV-FISTS-OF-IRONWOOD", Zone::Hand)
        .expect("Aura enters hand");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("fixture mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: fists,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Fists casts on creature");
    let first = game.priority;
    game.pass_priority(first).expect("caster passes Aura");
    let second = game.priority;
    game.pass_priority(second)
        .expect("Aura resolves and trigger stacks");

    assert_eq!(game.zone_of(fists), Some(Zone::Battlefield));
    assert_eq!(
        game.object(fists).expect("Aura exists").attached_to,
        Some(creature)
    );
    assert!(
        game.characteristics(creature)
            .expect("creature characteristics")
            .keywords
            .contains(&Keyword::Trample),
        "the attachment grants Trample before its ETB trigger resolves"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == fists && *ability == "etb-two-saprolings"
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::TokenCreated { .. }))
            .count(),
        0,
        "tokens are not created inline while the Aura spell resolves"
    );

    let first = game.priority;
    game.pass_priority(first)
        .expect("trigger controller passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("Saproling trigger resolves");
    println!(
        "fists_of_ironwood_event_log={:#?}",
        game.canonical_event_log()
    );

    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)))
            .count(),
        2,
        "exactly two controller-owned Saprolings are created by the trigger"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == fists && *ability == "etb-two-saprolings"
    )));
    game.validate_invariants()
        .expect("Fists event trace preserves Aura and trigger invariants");
}
