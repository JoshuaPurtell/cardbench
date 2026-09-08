//! Full-fidelity contracts for Cerulean Sphinx's owner-relative activation.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, ContinuousChange, Duration, Effect, Game, GameEvent,
    ManaCost, PlayerId, Target, Zone,
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

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != cardbench_magic_engine::Step::PrecombatMain {
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first response pass");
    let second = game.priority;
    game.pass_priority(second).expect("second response pass");
}

fn activate_shuffle(game: &mut Game, player: PlayerId, source: cardbench_magic_engine::ObjectId) {
    game.activate_ability(
        player,
        AbilityActivation {
            source,
            ability_id: "shuffle-source-into-owner-library",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Cerulean Sphinx shuffle activation is legal");
}

#[test]
fn cerulean_sphinx_definition_and_binding_are_complete() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CERULEAN-SPHINX")
        .expect("Cerulean Sphinx definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Blue, Color::Blue])
    );
    assert_eq!(definition.effects, Vec::<Effect>::new());

    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-CERULEAN-SPHINX"
                && binding.ability.id == "shuffle-source-into-owner-library"
        })
        .expect("Cerulean Sphinx activation exists");
    assert_eq!(
        binding.ability.mana_cost,
        ManaCost::with_colors(0, [Color::Blue])
    );
    assert!(!binding.ability.tap_cost);
    assert!(binding.ability.targets.is_empty());
    assert_eq!(
        binding.ability.effects,
        vec![Effect::MoveSourceToOwnersLibraryAndShuffle]
    );
}

#[test]
fn cerulean_sphinx_moves_to_its_owner_library_then_shuffles_that_library() {
    let mut game = game_with_rav_bindings();
    let sphinx = game
        .put_on_battlefield(PlayerId(0), "RAV-CERULEAN-SPHINX")
        .expect("Sphinx setup");
    let library_card = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("owner library setup");
    let island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("activation mana source setup");
    game.begin_game().expect("game starts");
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    game.activate_mana_ability(PlayerId(0), island, Color::Blue)
        .expect("activation mana is available");
    activate_shuffle(&mut game, PlayerId(0), sphinx);
    resolve_top(&mut game);

    println!(
        "Cerulean Sphinx owner shuffle trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(sphinx), Some(Zone::Library));
    assert!(game.players[PlayerId(0).0].library.contains(&sphinx));
    assert!(game.players[PlayerId(0).0].library.contains(&library_card));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled {
            player: PlayerId(0),
            cards: 2,
        }
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled {
            player: PlayerId(1),
            ..
        }
    )));
    game.validate_invariants()
        .expect("owner-relative source shuffle leaves an auditable state");
}

#[test]
fn cerulean_sphinx_uses_its_owner_library_after_control_changes() {
    let mut game = game_with_rav_bindings();
    let sphinx = game
        .put_on_battlefield(PlayerId(0), "RAV-CERULEAN-SPHINX")
        .expect("owner Sphinx setup");
    let control_marker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("control-effect provenance setup");
    let owner_library_card = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("owner library setup");
    let controller_library_card = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Library)
        .expect("temporary-controller library setup");
    let controller_island = game
        .put_on_battlefield(PlayerId(1), "RAV-ISLAND")
        .expect("temporary-controller activation mana source setup");
    game.begin_game().expect("game starts");
    advance_to_precombat_main(&mut game);
    game.add_continuous_effect(
        control_marker,
        sphinx,
        ContinuousChange::ChangeController(PlayerId(1)),
        Duration::EndOfTurn(game.turn),
    )
    .expect("temporary control effect installs");
    assert_eq!(game.controller_of(sphinx), Ok(PlayerId(1)));
    game.clear_event_log();

    game.pass_priority(PlayerId(0))
        .expect("temporary controller receives priority");
    game.activate_mana_ability(PlayerId(1), controller_island, Color::Blue)
        .expect("temporary-controller activation mana is available");
    activate_shuffle(&mut game, PlayerId(1), sphinx);
    resolve_top(&mut game);

    println!(
        "Cerulean Sphinx control-change trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(sphinx), Some(Zone::Library));
    assert!(game.players[PlayerId(0).0].library.contains(&sphinx));
    assert!(
        game.players[PlayerId(0).0]
            .library
            .contains(&owner_library_card)
    );
    assert!(
        game.players[PlayerId(1).0]
            .library
            .contains(&controller_library_card)
    );
    assert!(!game.players[PlayerId(1).0].library.contains(&sphinx));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled {
            player: PlayerId(0),
            cards: 2,
        }
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled {
            player: PlayerId(1),
            ..
        }
    )));
    game.validate_invariants()
        .expect("control-changed source still preserves owner-indexed libraries");
}

#[test]
fn cerulean_sphinx_departure_response_leaves_old_ability_auditable_no_op() {
    let mut game = game_with_rav_bindings();
    let sphinx = game
        .put_on_battlefield(PlayerId(0), "RAV-CERULEAN-SPHINX")
        .expect("Sphinx setup");
    let removal = game
        .add_card(PlayerId(1), "RAV-PUTREFY", Zone::Hand)
        .expect("response setup");
    let activation_island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("activation mana source setup");
    let response_swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("response black mana source setup");
    let response_forest = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("response green mana source setup");
    let response_generic = game.put_on_battlefield(PlayerId(1), "RAV-FOREST").unwrap();
    game.begin_game().expect("game starts");
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    game.activate_mana_ability(PlayerId(0), activation_island, Color::Blue)
        .expect("activation mana is available");
    activate_shuffle(&mut game, PlayerId(0), sphinx);
    game.pass_priority(PlayerId(0))
        .expect("opponent receives response priority");
    game.activate_mana_ability(PlayerId(1), response_swamp, Color::Black)
        .expect("response black mana is available");
    game.activate_mana_ability(PlayerId(1), response_forest, Color::Green)
        .expect("response green mana is available");
    game.activate_mana_ability(PlayerId(1), response_generic, Color::Green).unwrap();
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(sphinx)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Putrefy can remove the Sphinx in response");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(sphinx), Some(Zone::Graveyard));
    resolve_top(&mut game);

    println!(
        "Cerulean Sphinx departed-source trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(sphinx), Some(Zone::Graveyard));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled {
            player: PlayerId(0),
            ..
        }
    )));
    game.validate_invariants()
        .expect("departed source ability cannot move or shuffle a later incarnation");
}
