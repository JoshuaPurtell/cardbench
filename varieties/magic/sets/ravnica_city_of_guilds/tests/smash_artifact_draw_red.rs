//! Red discovery contract for Smash's artifact-destruction and draw behavior.
//!
//! This public probe intentionally checks the semantic card boundary (artifact
//! target, destruction, then one controller draw) without copying printed
//! Oracle text or card art.

use cardbench_magic_engine::{
    CardType, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game() -> Game {
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

#[test]
fn smash_definition_declares_artifact_destruction_and_draw() {
    let smash = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SMASH")
        .expect("Smash definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&smash.id));
    assert_eq!(smash.name, "Smash");
    assert_eq!(smash.mana_cost, ManaCost::with_colors(2, [Color::Red]));
    assert_eq!(smash.card_types, [CardType::Instant].into());
    assert_eq!(
        smash.effects,
        vec![Effect::DestroyTargetArtifact, Effect::DrawController]
    );
    assert!(smash.supported_rules.contains(&"artifact-destruction"));
    assert!(smash.supported_rules.contains(&"draw"));
}

#[test]
fn smash_destroys_an_artifact_then_draws_one_card() {
    let mut game = game();
    let smash = game
        .add_card(PlayerId(0), "RAV-SMASH", Zone::Hand)
        .expect("Smash enters hand");
    let artifact = game
        .put_on_battlefield(PlayerId(1), "RAV-BOROS-SIGNET")
        .expect("artifact enters battlefield");
    let drawn = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("draw card enters library");
    game.grant_mana(PlayerId(0), Color::Red, 3)
        .expect("Smash mana");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: smash,
            targets: vec![Target::Permanent(artifact)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Smash casts at artifact");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Smash resolves");

    println!("Smash artifact-draw trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(artifact), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { card, .. } if *card == artifact
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Hand } if *card == drawn
    )));
    game.validate_invariants().expect("Smash trace is valid");
}

#[test]
fn smash_rejects_a_nonartifact_target_at_cast_time() {
    let mut game = game();
    let smash = game
        .add_card(PlayerId(0), "RAV-SMASH", Zone::Hand)
        .expect("Smash enters hand");
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("creature enters battlefield");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("Smash mana");
    let result = game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: smash,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    println!("Smash nonartifact-target result: {result:?}");
    println!("Smash nonartifact-target events: {:?}", game.event_log);
    assert!(result.is_err());
    assert_eq!(game.zone_of(smash), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
}
