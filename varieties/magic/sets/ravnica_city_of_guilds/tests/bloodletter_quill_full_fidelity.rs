//! Event-log contract for Bloodletter Quill's ordered counter activations.

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, Color, CounterKind, Game, GameEvent,
    GeneralizedAbilityActivation, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_mana_ability_bindings,
    rav_replacement_effect_bindings,
};

fn game() -> Game {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .expect("Quill source-counter cost registers");
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .expect("replacement bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn bloodletter_quill_orders_counter_draw_dynamic_loss_and_counter_cost_receipts() {
    let mut game = game();
    let quill = game
        .put_on_battlefield(PlayerId(0), "RAV-BLOODLETTER-QUILL")
        .expect("Quill setup");
    game.put_on_battlefield(PlayerId(0), "RAV-DOUBLING-SEASON")
        .expect("Doubling Season makes the dynamic counter total observable");
    let drawn = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("library draw setup");
    game.begin_game().expect("fixture begins");
    game.add_mana_from_action(PlayerId(0), Color::Colorless, 2)
        .expect("two generic mana funds the draw activation");
    game.clear_event_log();

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: quill,
            ability_id: "two-tap-add-blood-draw-lose-for-blood",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("paid Quill draw ability stacks");
    pass_pair(&mut game);

    assert!(game.object(quill).expect("Quill remains live").tapped);
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 18);
    assert_eq!(
        game.object(quill)
            .expect("Quill remains live")
            .counters
            .get(&CounterKind::Named("blood")),
        Some(&2),
        "Doubling Season replaces the one blood-counter placement before the dynamic loss"
    );
    let placement = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::CounterPlaced { source, card, counter: CounterKind::Named("blood"), amount: 2 }
                if *source == quill && *card == quill
        ))
        .expect("doubled blood placement receipt");
    let draw = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == drawn))
        .expect("draw movement receipt");
    let loss = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::LifeLost { source, player: PlayerId(0), amount: 2 } if *source == quill
            )
        })
        .expect("dynamic two-life loss receipt");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SourceCounterLifeLoss {
            source,
            player: PlayerId(0),
            counter: CounterKind::Named("blood"),
            amount: 2,
            ..
        } if *source == quill
    )));
    assert!(
        placement < draw && draw < loss,
        "Quill effects retain printed order"
    );
    eprintln!(
        "bloodletter_quill_draw_trace={:?}",
        game.canonical_event_log()
    );

    game.add_mana_from_action(PlayerId(0), Color::Blue, 1)
        .expect("blue pays removal activation");
    game.add_mana_from_action(PlayerId(0), Color::Black, 1)
        .expect("black pays removal activation");
    game.clear_event_log();
    game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source: quill,
                ability_id: "blue-black-remove-blood",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment {
                counter_sources: vec![quill],
                return_permanents: vec![],
                chosen_x: None,
            },
            mana_payment_selection: None,
        },
    )
    .expect("source-counter cost is paid atomically before the stack receipt");
    pass_pair(&mut game);

    assert_eq!(
        game.object(quill)
            .expect("Quill remains live")
            .counters
            .get(&CounterKind::Named("blood")),
        Some(&1)
    );
    let cost = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::CounterRemovedAsAbilityCost {
                player: PlayerId(0), source, card, counter: CounterKind::Named("blood"), amount: 1, ..
            } if *source == quill && *card == quill
        ))
        .expect("counter-cost provenance receipt");
    let removal = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::CounterRemoved { source, card, counter: CounterKind::Named("blood"), amount: 1 }
                if *source == quill && *card == quill
        ))
        .expect("counter removal mutation receipt");
    let stacked = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityActivated { source, ability, .. }
                    if *source == quill && *ability == "blue-black-remove-blood"
            )
        })
        .expect("ability reaches the stack only after cost payment");
    assert!(cost < removal && removal < stacked);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-BLOODLETTER-QUILL"));
    eprintln!("bloodletter_quill_trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Quill counter lifecycle remains invariant-valid");
}

#[test]
fn departed_quill_draws_but_has_no_retained_counter_total_for_life_loss() {
    let mut game = game();
    let quill = game
        .put_on_battlefield(PlayerId(0), "RAV-BLOODLETTER-QUILL")
        .expect("Quill setup");
    let drawn = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("Quill draw setup");
    let smash = game
        .add_card(PlayerId(1), "RAV-SMASH", Zone::Hand)
        .expect("response setup");
    game.add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("Smash draw setup keeps the response controller alive");
    game.begin_game().expect("fixture begins");
    game.add_mana_from_action(PlayerId(0), Color::Colorless, 2)
        .expect("Quill activation mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: quill,
            ability_id: "two-tap-add-blood-draw-lose-for-blood",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Quill draw ability stacks");
    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority over Quill ability");
    game.add_mana_from_action(PlayerId(1), Color::Colorless, 2)
        .expect("Smash generic mana");
    game.add_mana_from_action(PlayerId(1), Color::Red, 1)
        .expect("Smash red mana");
    game.cast_spell(
        PlayerId(1),
        cardbench_magic_engine::CastRequest {
            card: smash,
            targets: vec![cardbench_magic_engine::Target::Permanent(quill)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Smash response stacks above Quill ability");
    pass_pair(&mut game);
    pass_pair(&mut game);

    assert_eq!(game.zone_of(quill), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 20);
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::LifeLost { source, .. } if *source == quill
        )),
        "a departed source has no retained battlefield counter total"
    );
    eprintln!(
        "bloodletter_quill_departed_source_trace={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("departed Quill resolution remains invariant-valid");
}
