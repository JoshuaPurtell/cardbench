//! Event-log contract for Dream Leash's source-relative control attachment.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone};
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

fn pass_stack_object(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves the stack object");
}

#[test]
fn dream_leash_controls_an_enchanted_permanent_then_reverts_when_destroyed() {
    let mut game = game_with_rav_bindings();
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent permanent setup");
    let leash = game
        .add_card(PlayerId(0), "RAV-DREAM-LEASH", Zone::Hand)
        .expect("Dream Leash setup");
    let blue_sources = (0..5)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("Blue source setup")
        })
        .collect::<Vec<_>>();
    let seed_spark = game
        .add_card(PlayerId(1), "RAV-SEED-SPARK", Zone::Hand)
        .expect("instant response setup");
    let white_sources = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-PLAINS")
                .expect("White source setup")
        })
        .collect::<Vec<_>>();

    game.begin_game().expect("game begins");
    for _ in 0..2 {
        pass_stack_object(&mut game);
    }
    for island in blue_sources {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("Dream Leash mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: leash,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Dream Leash casts on an opponent permanent");
    pass_stack_object(&mut game);

    assert_eq!(game.zone_of(leash), Some(Zone::Battlefield));
    assert_eq!(game.controller_of(target), Ok(PlayerId(0)));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { source, target: changed, from, to }
            if *source == leash && *changed == target && *from == PlayerId(1) && *to == PlayerId(0)
    )));

    game.pass_priority(PlayerId(0))
        .expect("active player passes the empty stack");
    for plains in white_sources {
        game.activate_mana_ability(PlayerId(1), plains, Color::White)
            .expect("response mana");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: seed_spark,
            targets: vec![Target::Permanent(leash)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Seed Spark can destroy the controlling Aura");
    pass_stack_object(&mut game);
    println!("dream_leash_event_log={:#?}", game.canonical_event_log());

    assert_eq!(game.zone_of(leash), Some(Zone::Graveyard));
    assert_eq!(game.controller_of(target), Ok(PlayerId(1)));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { source, target: changed, from, to }
            if *source == leash && *changed == target && *from == PlayerId(0) && *to == PlayerId(1)
    )));
    game.validate_invariants()
        .expect("Dream Leash control lifecycle preserves invariants");
}

#[test]
fn dream_leash_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-DREAM-LEASH"));
}

#[test]
fn dream_leash_derives_control_from_its_live_source_instead_of_its_caster() {
    let mut game = game_with_rav_bindings();
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent creature setup");
    let first_leash = game
        .add_card(PlayerId(0), "RAV-DREAM-LEASH", Zone::Hand)
        .expect("first Dream Leash setup");
    game.enter_attachment_without_cast(first_leash, creature)
        .expect("first Leash controls creature");
    assert_eq!(game.controller_of(creature), Ok(PlayerId(0)));

    let second_leash = game
        .add_card(PlayerId(1), "RAV-DREAM-LEASH", Zone::Hand)
        .expect("second Dream Leash setup");
    game.enter_attachment_without_cast(second_leash, first_leash)
        .expect("second Leash can control the first Aura as an enchanted permanent");

    assert_eq!(game.controller_of(first_leash), Ok(PlayerId(1)));
    assert_eq!(
        game.controller_of(creature),
        Ok(PlayerId(1)),
        "the first Leash's control effect follows its live source controller"
    );
    println!(
        "dream_leash_source_controller_event_log={:#?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { source, target, from, to }
            if *source == first_leash
                && *target == creature
                && *from == PlayerId(0)
                && *to == PlayerId(1)
    )));
    game.validate_invariants()
        .expect("nested source-relative control remains acyclic and valid");
}
