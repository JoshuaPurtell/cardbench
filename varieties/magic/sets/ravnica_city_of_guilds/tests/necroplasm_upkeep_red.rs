//! Red discovery contract for Necroplasm's two beginning-of-upkeep abilities.
//!
//! Necroplasm's controller orders its simultaneous counter and mana-value
//! sweep triggers.  The sweep therefore has to read its source's current
//! `+1/+1` counter quantity when it resolves, rather than being a
//! card-specific precomputed value.

use cardbench_magic_engine::{
    CastRequest, Color, CounterKind, DecisionKind, DecisionSelection, Effect, Game, GameEvent,
    ManaCost, PlayerId, Target, Zone,
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
fn necroplasm_requires_both_upkeep_bindings_and_counter_sweep_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NECROPLASM")
        .expect("Necroplasm definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(
            1,
            [
                cardbench_magic_engine::Color::Black,
                cardbench_magic_engine::Color::Black
            ]
        )
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Necroplasm is full only once its departed-source counter value is preserved"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"source-departure-last-known-counter-value"),
    );
    assert!(
        definition
            .supported_rules
            .contains(&"upkeep-add-plus-one-counter"),
        "Necroplasm must expose its first beginning-of-upkeep ability"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"upkeep-destroy-creatures-by-plus-one-counter-mana-value"),
        "Necroplasm must expose its second beginning-of-upkeep ability"
    );

    let bindings = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == definition.id)
        .map(|binding| binding.ability)
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2, "both simultaneous upkeep triggers bind");
    assert!(bindings.iter().any(|ability| {
        ability.id == "upkeep-add-plus-one-counter"
            && ability.effects
                == [Effect::AddCountersToSource {
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 1,
                }]
    }));
    assert!(bindings.iter().any(|ability| {
        ability.id == "upkeep-destroy-creatures-by-plus-one-counter-mana-value"
            && ability.effects
                == [
                    Effect::DestroyAllCreaturesWithManaValueEqualToSourceCounters {
                        counter: CounterKind::PlusOnePlusOne,
                    },
                ]
    }));

    let mut game = game();
    game.put_on_battlefield(PlayerId(0), definition.id)
        .expect("Necroplasm starts on battlefield");
    game.begin_game()
        .expect("game reaches the first upkeep trigger boundary");
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view is available")
            .pending_decision
            .is_some(),
        "the controller must order both simultaneous upkeep triggers before priority"
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn necroplasm_sweep_reads_the_counter_after_the_controller_orders_upkeep_triggers() {
    let mut game = game();
    let necroplasm = game
        .put_on_battlefield(PlayerId(0), "RAV-NECROPLASM")
        .expect("Necroplasm begins on battlefield");
    let mana_value_one = game
        .put_on_battlefield(PlayerId(0), "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("one-mana creature begins on battlefield");
    let mana_value_two = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("two-mana creature begins on battlefield");
    game.begin_game().expect("game reaches the first upkeep");

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view is available")
        .pending_decision
        .expect("Necroplasm's simultaneous triggers require an order");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
    let sweep = *decision
        .trigger_candidates
        .iter()
        .find(|entry| entry.ability == "upkeep-destroy-creatures-by-plus-one-counter-mana-value")
        .expect("sweep trigger is an orderable candidate");
    let counter = *decision
        .trigger_candidates
        .iter()
        .find(|entry| entry.ability == "upkeep-add-plus-one-counter")
        .expect("counter trigger is an orderable candidate");
    game.submit_decision(
        PlayerId(0),
        decision.id,
        // The first selected member goes lower on the stack.  Put the sweep
        // there so the counter trigger resolves first and its new count is
        // read by the later sweep resolution.
        DecisionSelection::TriggerOrder(vec![sweep, counter]),
    )
    .expect("controller orders counter to resolve before the sweep");
    pass_pair(&mut game);
    pass_pair(&mut game);

    println!(
        "Necroplasm ordered-upkeep trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(necroplasm), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(mana_value_one), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(mana_value_two), Some(Zone::Battlefield));
    assert_eq!(
        game.object(necroplasm)
            .expect("Necroplasm remains live")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card }
            if *source == necroplasm && *card == mana_value_one
    )));
    game.validate_invariants()
        .expect("the resolution-time counter sweep preserves invariants");
}

#[test]
fn necroplasm_sweep_uses_last_known_counter_value_after_source_departure() {
    let mut game = game();
    let necroplasm = game
        .put_on_battlefield(PlayerId(0), "RAV-NECROPLASM")
        .expect("Necroplasm begins on battlefield");
    let mana_value_one = game
        .put_on_battlefield(PlayerId(1), "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("one-mana creature begins on battlefield");
    let mana_value_two = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("two-mana creature begins on battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("black mana source begins on battlefield");
    let printed_cost_generic = game.put_on_battlefield(PlayerId(0), "RAV-SWAMP").unwrap();
    let last_gasp = game
        .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
        .expect("Last Gasp begins in hand");
    game.begin_game().expect("game reaches the first upkeep");

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view is available")
        .pending_decision
        .expect("Necroplasm's simultaneous triggers require an order");
    let sweep = *decision
        .trigger_candidates
        .iter()
        .find(|entry| entry.ability == "upkeep-destroy-creatures-by-plus-one-counter-mana-value")
        .expect("sweep trigger is orderable");
    let counter = *decision
        .trigger_candidates
        .iter()
        .find(|entry| entry.ability == "upkeep-add-plus-one-counter")
        .expect("counter trigger is orderable");
    game.submit_decision(
        PlayerId(0),
        decision.id,
        // The counter resolves first and makes the source's last known
        // +1/+1-counter total one before the response removes it.
        DecisionSelection::TriggerOrder(vec![sweep, counter]),
    )
    .expect("controller orders counter above sweep");
    pass_pair(&mut game);

    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("the controller can make black mana while the sweep is pending");
    game.activate_mana_ability(PlayerId(0), printed_cost_generic, Color::Black).unwrap();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: last_gasp,
            targets: vec![Target::Permanent(necroplasm)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Last Gasp responds before the pending sweep resolves");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(necroplasm), Some(Zone::Graveyard));

    pass_pair(&mut game);
    println!(
        "Necroplasm departed-source counter trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.zone_of(mana_value_one),
        Some(Zone::Graveyard),
        "the sweep must use the departed source's last known one-counter value"
    );
    assert_eq!(game.zone_of(mana_value_two), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SourceCounterValueMaterialized {
            source,
            ability,
            counter: CounterKind::PlusOnePlusOne,
            amount: 1,
            ..
        } if *source == necroplasm
            && *ability == "upkeep-destroy-creatures-by-plus-one-counter-mana-value"
    )));
    game.validate_invariants()
        .expect("departed-source counter provenance remains invariant-valid");
}
