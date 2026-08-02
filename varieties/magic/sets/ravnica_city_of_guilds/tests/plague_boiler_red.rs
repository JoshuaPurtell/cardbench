//! Red discovery contract for Plague Boiler's counter-backed upkeep and sweep.
//!
//! The activation samples its source's plague-counter quantity before the
//! artifact is sacrificed as a cost.  The resolving stack ability must retain
//! that last-known value: reading counters from the departed card would either
//! lose the value at zone change or accidentally inspect a later incarnation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CounterKind, Effect, Game, GameEvent, ManaCost, PlayerId,
    Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

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

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn plague_boiler_requires_its_exact_counter_and_sacrifice_contract() {
    let boiler = definition("RAV-PLAGUE-BOILER");
    assert_eq!(boiler.name, "Plague Boiler");
    assert_eq!(boiler.mana_cost, ManaCost::new(1));
    assert_eq!(boiler.colors, BTreeSet::<Color>::new());
    assert_eq!(boiler.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&boiler.id),
        "Plague Boiler is full only when both its upkeep and exact last-known counter sweep are represented"
    );
    assert!(
        boiler
            .supported_rules
            .contains(&"upkeep-add-plague-counter")
    );
    assert!(
        boiler
            .supported_rules
            .contains(&"activated-sacrifice-sweep-nonlands-by-plague-counters")
    );

    let upkeep = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == boiler.id)
        .expect("Plague Boiler upkeep trigger exists")
        .ability;
    assert_eq!(upkeep.id, "upkeep-add-plague-counter");
    assert_eq!(
        upkeep.effects,
        [Effect::AddCountersToSource {
            counter: CounterKind::Named("plague"),
            amount: 1,
        }]
    );

    let activation = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == boiler.id)
        .expect("Plague Boiler sacrifice ability exists")
        .ability;
    assert_eq!(
        activation.id,
        "one-sacrifice-sweep-nonlands-by-plague-counters"
    );
    assert_eq!(activation.mana_cost, ManaCost::new(1));
    assert!(activation.sacrifice_source);
    assert!(!activation.tap_cost);
    assert!(activation.targets.is_empty());
    assert_eq!(
        activation.effects,
        [
            Effect::DestroyAllNonlandPermanentsWithManaValueEqualToSourceCounters {
                counter: CounterKind::Named("plague"),
            }
        ]
    );
}

#[test]
fn plague_boiler_retains_its_upkeep_counter_across_the_sacrifice_cost() {
    let mut game = game();
    let boiler = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAGUE-BOILER")
        .expect("Plague Boiler starts on battlefield");
    let mana_value_one = game
        .put_on_battlefield(PlayerId(0), "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("one-mana nonland begins on battlefield");
    let mana_value_two = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("two-mana nonland begins on battlefield");
    let land = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("land begins on battlefield");
    game.begin_game().expect("game begins at first upkeep");

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == boiler && *ability == "upkeep-add-plague-counter"
    )));
    pass_pair(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced { source, card, counter, amount }
            if *source == boiler && *card == boiler && *counter == CounterKind::Named("plague") && *amount == 1
    )));

    game.add_mana_from_action(PlayerId(0), Color::Colorless, 1)
        .expect("one generic mana is a legal priority action");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: boiler,
            ability_id: "one-sacrifice-sweep-nonlands-by-plague-counters",
            sacrifice_sources: vec![boiler],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Boiler pays one and is sacrificed before its ability resolves");
    assert_eq!(game.zone_of(boiler), Some(Zone::Graveyard));
    assert!(matches!(
        game.stack.last().map(|item| item.effects.as_slice()),
        Some([Effect::DestroyAllNonlandPermanentsWithManaValue { mana_value: 1 }])
    ));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SourceCounterValueMaterialized {
            source,
            ability,
            counter,
            amount,
            ..
        } if *source == boiler
            && *ability == "one-sacrifice-sweep-nonlands-by-plague-counters"
            && *counter == CounterKind::Named("plague")
            && *amount == 1
    )));
    pass_pair(&mut game);

    println!("Plague Boiler trace: {:#?}", game.canonical_event_log());
    assert_eq!(game.zone_of(mana_value_one), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(mana_value_two), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(land), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card }
            if *source == boiler && *card == mana_value_one
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == boiler && *ability == "one-sacrifice-sweep-nonlands-by-plague-counters"
    )));
    game.validate_invariants()
        .expect("counter last-known information and the sweep preserve invariants");
}

#[test]
fn plague_boiler_failed_cost_does_not_leak_a_counter_snapshot_or_sacrifice() {
    let mut game = game();
    let boiler = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAGUE-BOILER")
        .expect("Plague Boiler starts on battlefield");
    game.begin_game().expect("game begins at first upkeep");
    pass_pair(&mut game);
    game.clear_event_log();

    let error = game
        .activate_ability(
            PlayerId(0),
            AbilityActivation {
                source: boiler,
                ability_id: "one-sacrifice-sweep-nonlands-by-plague-counters",
                sacrifice_sources: vec![boiler],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        )
        .expect_err("missing generic mana rejects the complete activation");
    assert!(format!("{error}").contains("missing generic mana"));
    assert_eq!(game.zone_of(boiler), Some(Zone::Battlefield));
    assert_eq!(
        game.object(boiler)
            .expect("Boiler remains live")
            .counters
            .get(&CounterKind::Named("plague")),
        Some(&1)
    );
    assert!(game.stack.is_empty());
    assert!(
        game.event_log.is_empty(),
        "atomic rejection rolls back receipts"
    );
    game.validate_invariants()
        .expect("rejected source-counter sweep remains invariant-valid");
}
