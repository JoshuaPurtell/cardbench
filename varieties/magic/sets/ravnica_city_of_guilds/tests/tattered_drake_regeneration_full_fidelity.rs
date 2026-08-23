//! Event-log contract for Tattered Drake's self-regeneration activation.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn tattered_drake_regenerates_from_lethal_damage() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV catalog constructs");
    let drake = game
        .put_on_battlefield(PlayerId(0), "RAV-TATTERED-DRAKE")
        .expect("Drake setup");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("black source");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("lethal damage setup");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
                .expect("red source")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");

    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Drake regeneration mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: drake,
            ability_id: "self-regeneration",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Drake regeneration activation");
    resolve_top(&mut game);
    game.pass_priority(PlayerId(0))
        .expect("Drake controller passes to Char caster");
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(1), mountain, Color::Red)
            .expect("Char mana");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(drake)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char targets the Drake");
    resolve_top(&mut game);

    println!(
        "tattered_drake_regeneration_log={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(drake), Some(Zone::Battlefield));
    assert!(game.object(drake).expect("Drake survives").tapped);
    assert_eq!(game.object(drake).expect("Drake survives").damage, 0);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == drake && *target == drake
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldUsed { source, target }
            if *source == drake && *target == drake
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { card, .. } if *card == drake
    )));
    game.validate_invariants()
        .expect("self-regeneration trace preserves invariants");
}
