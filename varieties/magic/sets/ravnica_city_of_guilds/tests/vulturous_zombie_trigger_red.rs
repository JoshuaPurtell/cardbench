//! Red regression for Vulturous Zombie's opponent-graveyard growth trigger.
//!
//! A discard from an opponent's hand is deliberately used here because the
//! engine observer must follow every ordinary opponent-owned move into a
//! graveyard, rather than only creature deaths on the battlefield.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionSelection, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

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
    .expect("RAV fixture builds")
}

#[test]
fn vulturous_zombie_grows_when_an_opponent_discards_a_card() {
    let mut game = game();
    let zombie = game
        .put_on_battlefield(PlayerId(0), "RAV-VULTUROUS-ZOMBIE")
        .expect("Zombie begins on the battlefield");
    let nightmare_void = game
        .add_card(PlayerId(0), "RAV-NIGHTMARE-VOID", Zone::Hand)
        .expect("discard spell begins in hand");
    let discarded = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Hand)
        .expect("opponent card begins in hand");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("fixture mana");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: nightmare_void,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("targeted discard spell casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and spell resolves to its private choice");
    let decision = game
        .view_for_player(PlayerId(1))
        .expect("opponent receives discard choice")
        .pending_decision
        .expect("discard decision opens");
    game.submit_decision(
        PlayerId(1),
        decision.id,
        DecisionSelection::Objects(vec![discarded]),
    )
    .expect("opponent discards its hand card");
    game.pass_priority(PlayerId(0))
        .expect("controller passes the queued trigger");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and queued trigger resolves");

    println!(
        "Vulturous Zombie red trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == zombie && *ability == "opponent-card-to-graveyard-plus-one-counter"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced { source, card, .. }
            if *source == zombie && *card == zombie
    )));
    assert_eq!(
        game.characteristics(zombie)
            .expect("Zombie remains on the battlefield")
            .power,
        Some(4),
        "the opponent-owned discard must add one +1/+1 counter to Vulturous Zombie"
    );
    game.validate_invariants()
        .expect("opponent-graveyard trigger trace remains valid");
}

#[test]
fn vulturous_zombie_is_not_full_fidelity_without_its_graveyard_trigger() {
    let zombie = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VULTUROUS-ZOMBIE")
        .expect("Vulturous Zombie definition exists");
    assert!(
        zombie
            .supported_rules
            .contains(&"opponent-card-to-graveyard-plus-one-counter"),
        "the explicit opponent-graveyard trigger must be represented"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&zombie.id),
        "the exact static characteristics plus graveyard trigger make the definition complete"
    );
}
