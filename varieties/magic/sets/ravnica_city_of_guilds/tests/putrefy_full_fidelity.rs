//! Full-fidelity contracts for Putrefy's typed destruction instruction.

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, Effect, Game, GameEvent, PlayerId, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, run_all_scenarios,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn putrefy_has_an_exact_typed_definition_and_target_requirement() {
    let spell = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PUTREFY")
        .expect("Putrefy definition exists");
    assert_eq!(
        executable_definition_id_for_collector(221),
        Ok("RAV-PUTREFY")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&spell.id));
    assert_eq!(spell.card_types, [CardType::Instant].into_iter().collect());
    assert_eq!(
        spell.colors,
        [Color::Black, Color::Green].into_iter().collect()
    );
    assert_eq!(
        spell.effects,
        [Effect::DestroyTargetArtifactOrCreatureNoRegeneration]
    );
    assert_eq!(
        spell.effects[0].target_requirement(),
        Some(TargetRequirement::ArtifactOrCreature)
    );
}

#[test]
fn putrefy_destroys_an_artifact_creature_without_consuming_its_regeneration_shield() {
    let mut game = game_with_rav_bindings();
    let votary = game
        .put_on_battlefield(PlayerId(0), "RAV-VOTARY-OF-THE-CONCLAVE")
        .expect("Votary begins on battlefield");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-GLASS-GOLEM")
        .expect("artifact creature begins on battlefield");
    let first_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("first Forest begins on battlefield");
    let second_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("second Forest begins on battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp begins on battlefield");
    let putrefy = game
        .add_card(PlayerId(0), "RAV-PUTREFY", Zone::Hand)
        .expect("Putrefy begins in hand");
    game.begin_game().expect("fixture begins game");

    game.activate_mana_ability(PlayerId(0), first_forest, Color::Green)
        .expect("Forest pays Votary activation");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: votary,
            ability_id: "regenerate-target-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Votary creates a regeneration shield");
    resolve_top(&mut game);

    game.activate_mana_ability(PlayerId(0), second_forest, Color::Green)
        .expect("Forest pays Putrefy");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp pays Putrefy");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: putrefy,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Putrefy accepts an artifact creature target");
    resolve_top(&mut game);

    println!(
        "Putrefy no-regeneration trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card } if *source == putrefy && *card == target
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldUsed { target: shielded, .. } if *shielded == target
    )));
    game.validate_invariants()
        .expect("Putrefy no-regeneration path preserves invariants");
}

#[test]
fn putrefy_public_scenario_records_destruction_without_regeneration_use() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_putrefy_bypasses_regeneration")
        .expect("Putrefy public scenario exists");
    println!("Putrefy public trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:8e2aa574113519a9");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("CardDestroyed"))
    );
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("RegenerationShieldUsed"))
    );
}
