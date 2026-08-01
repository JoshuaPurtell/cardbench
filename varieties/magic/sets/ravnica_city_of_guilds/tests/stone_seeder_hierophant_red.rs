//! Red regression for Stone-Seeder Hierophant's land-entry trigger and land untap activation.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, ManaCost, PlayerId, PolicyAction, Step,
    Target, TargetRequirement, Zone,
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
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward main phase");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward main phase");
    }
}

fn advance_to_second_players_first_main(game: &mut Game) {
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("first main priority pass");
        game.pass_priority(PlayerId(1))
            .expect("first main opposing pass");
    }
    game.declare_attackers(PlayerId(0), &[])
        .expect("empty attack declaration");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("first player passes through the remaining first turn");
        game.pass_priority(PlayerId(1))
            .expect("second player passes through the remaining first turn");
    }
    assert_eq!(game.active_player, PlayerId(1));
    assert_eq!(game.step, Step::Upkeep);
    game.pass_priority(PlayerId(1))
        .expect("second player passes upkeep");
    game.pass_priority(PlayerId(0))
        .expect("first player passes upkeep");
    assert_eq!(game.step, Step::Draw);
    game.draw_card(PlayerId(1), None)
        .expect("second player's ordinary draw resolves");
    game.pass_priority(PlayerId(1))
        .expect("second player passes draw priority");
    game.pass_priority(PlayerId(0))
        .expect("first player passes draw priority");
    assert_eq!(game.active_player, PlayerId(1));
    assert_eq!(game.step, Step::PrecombatMain);
}

#[test]
fn stone_seeder_hierophant_has_its_exact_trigger_and_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STONE-SEEDER-HIEROPHANT")
        .expect("Stone-Seeder Hierophant definition exists");
    assert_eq!(definition.name, "Stone-Seeder Hierophant");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Green, Color::Green])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(1)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.supported_rules.contains(&"landfall-untap"));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-untap-target-land")
    );

    let activation = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-STONE-SEEDER-HIEROPHANT"
                && binding.ability.id == "untap-target-land"
        })
        .expect("Stone-Seeder Hierophant activation exists")
        .ability;
    assert_eq!(activation.mana_cost, ManaCost::new(0));
    assert!(activation.tap_cost);
    assert_eq!(activation.targets, vec![TargetRequirement::Land]);
    assert!(rav_triggered_ability_bindings().into_iter().any(|binding| {
        binding.card_definition == "RAV-STONE-SEEDER-HIEROPHANT"
            && binding.ability.id == "landfall-untap-source"
    }));
}

#[test]
fn stone_seeder_hierophant_untaps_a_target_land_through_the_stack() {
    let mut game = game_with_rav_bindings();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-STONE-SEEDER-HIEROPHANT")
        .expect("Stone-Seeder Hierophant setup");
    game.set_entered_turn_for_setup(source, 0)
        .expect("source predates the first turn");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest setup");
    game.begin_game().expect("fixture starts game");
    advance_to_first_main(&mut game);
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest taps for mana");
    game.clear_event_log();

    game.submit_policy_move(
        PlayerId(0),
        "stone-seeder-untap-target-land",
        PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source,
                ability_id: "untap-target-land",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Permanent(forest)],
            },
        },
    )
    .expect("activation stacks");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert!(!game.object(forest).expect("Forest remains").tapped);
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::PermanentUntapped { source: event_source, card }
                if *event_source == source && *card == forest
        )
    }));
    game.validate_invariants()
        .expect("land untap resolution preserves invariants");
}

#[test]
fn stone_seeder_hierophant_landfall_uses_a_triggered_stack_object() {
    let mut game = game_with_rav_bindings();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-STONE-SEEDER-HIEROPHANT")
        .expect("Stone-Seeder Hierophant setup");
    game.set_entered_turn_for_setup(source, 0)
        .expect("source predates the first turn");
    let initial_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("initial Forest setup");
    let entering_forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Hand)
        .expect("entering Forest setup");
    game.begin_game().expect("fixture starts game");
    advance_to_first_main(&mut game);

    game.submit_policy_move(
        PlayerId(0),
        "stone-seeder-tap-source",
        PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source,
                ability_id: "untap-target-land",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Permanent(initial_forest)],
            },
        },
    )
    .expect("source tap activation stacks");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    assert!(game.object(source).expect("source remains").tapped);

    game.clear_event_log();
    game.play_land(PlayerId(0), entering_forest)
        .expect("land enters through the normal main-phase action");
    assert_eq!(game.stack.len(), 1, "landfall must use the stack");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert!(!game.object(source).expect("source remains").tapped);
    println!(
        "Stone-Seeder Hierophant landfall trace: {:?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source: event_source,
                ability: "landfall-untap-source",
                ..
            } if *event_source == source
        )
    }));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::PermanentUntapped { source: event_source, card }
                if *event_source == source && *card == source
        )
    }));
    game.validate_invariants()
        .expect("landfall resolution preserves invariants");
}

#[test]
fn stone_seeder_hierophant_observes_an_opponents_land_entry() {
    let mut game = game_with_rav_bindings();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-STONE-SEEDER-HIEROPHANT")
        .expect("Stone-Seeder Hierophant setup");
    game.set_entered_turn_for_setup(source, 0)
        .expect("source predates the first turn");
    let initial_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("initial Forest setup");
    let opponents_forest = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Hand)
        .expect("opponent land setup");
    game.add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("opponent draw setup");
    game.begin_game().expect("fixture starts game");
    advance_to_first_main(&mut game);

    game.submit_policy_move(
        PlayerId(0),
        "stone-seeder-tap-before-opponent-landfall",
        PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source,
                ability_id: "untap-target-land",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Permanent(initial_forest)],
            },
        },
    )
    .expect("source tap activation stacks");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    assert!(game.object(source).expect("source remains").tapped);

    advance_to_second_players_first_main(&mut game);
    game.clear_event_log();
    game.play_land(PlayerId(1), opponents_forest)
        .expect("opponent plays a normal land");
    assert_eq!(game.stack.len(), 1, "opponent land entry stacks landfall");
    game.pass_priority(PlayerId(1))
        .expect("active opponent passes the trigger");
    game.pass_priority(PlayerId(0))
        .expect("Stone-Seeder controller passes the trigger");

    assert!(!game.object(source).expect("source remains").tapped);
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                controller: PlayerId(0),
                source: event_source,
                ability: "landfall-untap-source",
            } if *event_source == source
        )
    }));
    game.validate_invariants()
        .expect("opponent land entry preserves state-machine invariants");
}
