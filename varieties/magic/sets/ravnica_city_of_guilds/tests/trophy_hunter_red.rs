//! Red discovery regression for Trophy Hunter's typed Flying destruction.

use cardbench_magic_engine::{
    AbilityActivation, Color, CounterKind, Effect, Game, GameEvent, ManaCost, ObjectId,
    PlayerId, Target, TargetRequirement, Zone,
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

fn pay_green(game: &mut Game) -> ObjectId {
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on battlefield");
    game.begin_game().expect("fixture begins game");
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest provides green mana");
    forest
}

#[test]
fn trophy_hunter_requires_flying_destruction_then_source_counter() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TROPHY-HUNTER")
        .expect("Trophy Hunter definition exists");
    assert!(
        definition
            .supported_rules
            .contains(&"activated-flying-destruction-source-counter"),
        "Trophy Hunter must declare its complete activated behavior"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "the typed activation and public trace make Trophy Hunter complete"
    );

    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-TROPHY-HUNTER"
                && binding.ability.id == "destroy-flying-add-plus-one-counter"
        })
        .expect("Trophy Hunter activation binding exists");
    assert_eq!(binding.ability.mana_cost, ManaCost::with_colors(0, [Color::Green]));
    assert!(!binding.ability.tap_cost);
    assert_eq!(binding.ability.targets, [TargetRequirement::FlyingCreature]);
    assert_eq!(
        binding.ability.effects,
        [
            Effect::DestroyTargetFlyingCreature,
            Effect::AddPlusOneCounterToSource,
        ]
    );

    let mut game = game_with_rav_bindings();
    let hunter = game
        .put_on_battlefield(PlayerId(0), "RAV-TROPHY-HUNTER")
        .expect("Hunter begins on battlefield");
    let flyer = game
        .put_on_battlefield(PlayerId(1), "RAV-BIRDS-OF-PARADISE")
        .expect("flying target begins on battlefield");
    pay_green(&mut game);
    game.clear_event_log();
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: hunter,
            ability_id: "destroy-flying-add-plus-one-counter",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(flyer)],
        },
    )
    .expect("Hunter ability enters the stack");
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
    println!("trophy_hunter_event_log={:?}", game.canonical_event_log());

    assert_eq!(game.zone_of(flyer), Some(Zone::Graveyard));
    assert_eq!(
        game.object(hunter)
            .expect("Hunter remains live")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert_eq!(
        (
            game.characteristics(hunter).expect("Hunter characteristics").power,
            game.characteristics(hunter)
                .expect("Hunter characteristics")
                .toughness,
        ),
        (Some(3), Some(4))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card } if *source == hunter && *card == flyer
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced {
            source,
            card,
            counter: CounterKind::PlusOnePlusOne,
            amount: 1,
        } if *source == hunter && *card == hunter
    )));
    game.validate_invariants()
        .expect("Trophy Hunter trace preserves invariants");
}

#[test]
fn trophy_hunter_rejects_nonflying_target_before_mana_payment() {
    let mut game = game_with_rav_bindings();
    let hunter = game
        .put_on_battlefield(PlayerId(0), "RAV-TROPHY-HUNTER")
        .expect("Hunter begins on battlefield");
    let nonflyer = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("nonflying target begins on battlefield");
    pay_green(&mut game);
    game.clear_event_log();
    let mana_before = game.player(PlayerId(0)).expect("player exists").mana_pool.clone();
    let result = game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: hunter,
            ability_id: "destroy-flying-add-plus-one-counter",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(nonflyer)],
        },
    );
    assert!(result.is_err(), "a nonflying target is illegal");
    assert_eq!(game.player(PlayerId(0)).expect("player exists").mana_pool, mana_before);
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected Hunter activation preserves invariants");
}
