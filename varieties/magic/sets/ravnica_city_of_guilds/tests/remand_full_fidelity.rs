//! Event-log contract for Remand's any-spell counter and draw sequence.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, RulesError, Target, Zone,
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

#[test]
fn remand_counters_a_creature_spell_then_draws_its_controller() {
    let mut game = game_with_rav_bindings();
    let watchwolf = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("creature spell setup");
    let remand = game
        .add_card(PlayerId(1), "RAV-REMAND", Zone::Hand)
        .expect("Remand setup");
    let drawn = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
        .expect("draw setup");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("white source");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("green source");
    let blue_sources = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-ISLAND")
                .expect("blue source")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first phase pass");
        let second = game.priority;
        game.pass_priority(second).expect("second phase pass");
    }
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("white creature mana");
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("green creature mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Watchwolf casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    for island in blue_sources {
        game.activate_mana_ability(PlayerId(1), island, Color::Blue)
            .expect("Remand mana");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: remand,
            targets: vec![Target::Spell(watchwolf)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Remand targets the creature spell");
    game.pass_priority(PlayerId(1))
        .expect("Remand controller passes");
    game.pass_priority(PlayerId(0)).expect("Remand resolves");
    println!("remand_event_log={:#?}", game.canonical_event_log());

    assert_eq!(game.zone_of(watchwolf), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(event, GameEvent::SpellCountered { card, source } if *card == watchwolf && *source == remand)));
    game.validate_invariants()
        .expect("Remand trace preserves invariants");
}

#[test]
fn remand_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-REMAND"));
}

#[test]
fn remand_cannot_target_an_activated_ability() {
    let mut game = game_with_rav_bindings();
    let forgeling = game
        .put_on_battlefield(PlayerId(0), "RAV-GREATER-FORGELING")
        .expect("activated-ability source setup");
    game.set_entered_turn_for_setup(forgeling, 0)
        .expect("old fixture entry");
    let remand = game
        .add_card(PlayerId(1), "RAV-REMAND", Zone::Hand)
        .expect("Remand setup");
    let red_sources = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("red source")
        })
        .collect::<Vec<_>>();
    let blue_sources = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-ISLAND")
                .expect("blue source")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first phase pass");
        let second = game.priority;
        game.pass_priority(second).expect("second phase pass");
    }
    for mountain in red_sources {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("Forgeling mana");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: forgeling,
            ability_id: "pump-plus-three-minus-three",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Forgeling ability activates");
    game.pass_priority(PlayerId(0))
        .expect("activator passes to Remand controller");
    for island in blue_sources {
        game.activate_mana_ability(PlayerId(1), island, Color::Blue)
            .expect("Remand mana");
    }

    let event_count = game.event_log.len();
    assert_eq!(
        game.cast_spell(
            PlayerId(1),
            CastRequest {
                card: remand,
                targets: vec![Target::Spell(forgeling)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        ),
        Err(RulesError::IllegalTarget(Target::Spell(forgeling))),
    );
    assert_eq!(game.event_log.len(), event_count);
    game.validate_invariants()
        .expect("rejected ability target leaves the game valid");
}
