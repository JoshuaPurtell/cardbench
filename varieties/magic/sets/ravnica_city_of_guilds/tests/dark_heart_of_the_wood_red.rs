//! Red discovery contract for Dark Heart of the Wood's typed Forest-sacrifice activation.

use cardbench_magic_engine::{
    AbilityActivation, BasicLandType, CardType, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_mana_ability_bindings,
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
    .expect("RAV game builds");
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .expect("typed Forest cost binding registers");
    game
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("controller passes");
    let second = game.priority;
    game.pass_priority(second).expect("opponent passes");
}

#[test]
fn dark_heart_of_the_wood_requires_its_exact_enchantment_definition() {
    let heart = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DARK-HEART-OF-THE-WOOD")
        .expect("Dark Heart of the Wood definition exists");

    assert_eq!(heart.name, "Dark Heart of the Wood");
    assert_eq!(heart.mana_cost, ManaCost::with_colors(0, [Color::Black, Color::Green]));
    assert_eq!(
        heart.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&heart.id),
        "Dark Heart is full only when its typed Forest-sacrifice activation is represented"
    );
    assert!(
        heart
            .supported_rules
            .contains(&"activated-green-sacrifice-forest-gain-three-life")
    );

    let ability = &rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == heart.id)
        .expect("Dark Heart activation exists")
        .ability;
    assert_eq!(ability.mana_cost, ManaCost::with_colors(0, [Color::Green]));
    assert_eq!(ability.sacrifice_lands, 1);
    assert_eq!(ability.effects, [Effect::GainLifeController { amount: 3 }]);
}

#[test]
fn dark_heart_rejects_a_nonforest_sacrifice_without_mutating_state() {
    let mut game = game();
    let heart = game
        .put_on_battlefield(PlayerId(0), "RAV-DARK-HEART-OF-THE-WOOD")
        .expect("Dark Heart setup");
    let payer = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest pays green cost");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains is an illegal sacrifice choice");
    game.begin_game().expect("game begins");
    game.activate_mana_ability(PlayerId(0), payer, Color::Green)
        .expect("Forest provides green mana");
    game.clear_event_log();

    let error = game
        .activate_ability(
            PlayerId(0),
            AbilityActivation {
                source: heart,
                ability_id: "green-sacrifice-forest-gain-three-life",
                sacrifice_sources: vec![plains],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        )
        .expect_err("a Plains cannot pay Dark Heart's Forest cost");
    assert!(format!("{error}").contains("bound basic-land type"));
    assert_eq!(game.zone_of(plains), Some(Zone::Battlefield));
    assert_eq!(game.players[0].mana_pool.amount(Color::Green), 1);
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected typed cost leaves a valid game");
}

#[test]
fn dark_heart_pays_one_exact_forest_then_gains_three_life_on_resolution() {
    let mut game = game();
    let heart = game
        .put_on_battlefield(PlayerId(0), "RAV-DARK-HEART-OF-THE-WOOD")
        .expect("Dark Heart setup");
    let payer = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest pays green cost");
    let sacrificed = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("controlled Forest is selected as cost");
    game.begin_game().expect("game begins");
    game.activate_mana_ability(PlayerId(0), payer, Color::Green)
        .expect("Forest provides green mana");
    game.clear_event_log();

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: heart,
            ability_id: "green-sacrifice-forest-gain-three-life",
            sacrifice_sources: vec![sacrificed],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("exact Forest payment is accepted");
    assert_eq!(game.zone_of(sacrificed), Some(Zone::Graveyard));
    assert_eq!(game.players[0].life, 20, "life waits for stack resolution");
    assert!(game.event_log.windows(3).any(|events| matches!(
        events,
        [
            GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability, .. },
            GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source: sacrifice_source, permanent },
            GameEvent::CardMoved { card, to: Zone::Graveyard },
        ] if *source == heart && *ability == "green-sacrifice-forest-gain-three-life"
            && *sacrifice_source == heart && *permanent == sacrificed && *card == sacrificed
    )));

    resolve_top(&mut game);
    assert_eq!(game.players[0].life, 23);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained {
            player: PlayerId(0),
            amount: 3,
            ..
        }
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability: "green-sacrifice-forest-gain-three-life", .. }
            if *source == heart
    )));
    assert_eq!(
        game.basic_land_type(sacrificed),
        Ok(Some(BasicLandType::Forest))
    );
    eprintln!("Dark Heart trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("typed Forest cost and life-gain stack lifecycle stay valid");
}
