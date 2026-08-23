//! Event-log contract for Mark of Eviction's source-relative upkeep trigger.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone};
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

#[test]
fn mark_returns_its_currently_enchanted_creature_then_sba_cleans_up_the_aura() {
    let mut game = game_with_rav_bindings();
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent creature setup");
    let mark = game
        .add_card(PlayerId(0), "RAV-MARK-OF-EVICTION", Zone::Hand)
        .expect("Mark setup");
    game.enter_attachment_without_cast(mark, creature)
        .expect("Mark attaches to a creature before the measured upkeep");

    game.begin_game()
        .expect("game begins at player zero upkeep");
    assert_eq!(game.step, Step::Upkeep);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == mark && *ability == "upkeep-return-enchanted-creature"
    )));

    game.pass_priority(PlayerId(0))
        .expect("Mark controller passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and Mark trigger resolves");
    println!(
        "mark_of_eviction_event_log={:#?}",
        game.canonical_event_log()
    );

    assert_eq!(game.zone_of(creature), Some(Zone::Hand));
    assert_eq!(game.zone_of(mark), Some(Zone::Graveyard));
    let returned = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == creature))
        .expect("enchanted creature has an owner-hand receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == mark && *ability == "upkeep-return-enchanted-creature"
            )
        })
        .expect("trigger resolves after its zone instruction");
    let cleanup = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::StateBasedAction { card, reason }
                    if *card == mark && *reason == "Aura is not attached to a battlefield creature"
            )
        })
        .expect("unattached Aura receives ordinary SBA cleanup");
    assert!(returned < resolved && resolved < cleanup);
    game.validate_invariants()
        .expect("source-relative upkeep trace preserves invariants");
}

#[test]
fn mark_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-MARK-OF-EVICTION"));
}

#[test]
fn departed_mark_cannot_return_a_former_attachment_when_its_trigger_resolves() {
    let mut game = game_with_rav_bindings();
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent creature setup");
    let mark = game
        .add_card(PlayerId(0), "RAV-MARK-OF-EVICTION", Zone::Hand)
        .expect("Mark setup");
    game.enter_attachment_without_cast(mark, creature)
        .expect("Mark attaches before the measured upkeep");
    let seed_spark = game
        .add_card(PlayerId(1), "RAV-SEED-SPARK", Zone::Hand)
        .expect("response spell setup");
    let plains = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-PLAINS")
                .expect("Plains setup")
        })
        .collect::<Vec<_>>();

    game.begin_game()
        .expect("game begins at player zero upkeep");
    game.pass_priority(PlayerId(0))
        .expect("Mark controller passes trigger");
    for plains in plains {
        game.activate_mana_ability(PlayerId(1), plains, Color::White)
            .expect("response mana");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: seed_spark,
            targets: vec![Target::Permanent(mark)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Seed Spark can remove Mark in response");
    game.pass_priority(PlayerId(1))
        .expect("Seed Spark controller passes");
    game.pass_priority(PlayerId(0))
        .expect("Seed Spark resolves above Mark trigger");
    assert_eq!(game.zone_of(mark), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));

    game.pass_priority(PlayerId(0))
        .expect("active player passes original trigger");
    game.pass_priority(PlayerId(1))
        .expect("departed Mark trigger resolves as a no-op");
    println!(
        "mark_of_eviction_departed_source_event_log={:#?}",
        game.canonical_event_log()
    );

    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::CardMoved { card, to: Zone::Hand } if *card == creature
        )),
        "a departed Aura cannot follow a former attachment into a later resolution"
    );
    game.validate_invariants()
        .expect("departed source leaves no stale attachment provenance");
}
