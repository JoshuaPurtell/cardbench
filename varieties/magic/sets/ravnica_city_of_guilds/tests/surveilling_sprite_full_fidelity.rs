//! Event-log contract for Surveilling Sprite's target-free dies trigger.

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

#[test]
fn surveilling_sprite_dies_then_draws_from_a_separate_trigger_stack_object() {
    let mut game = game_with_rav_bindings();
    let sprite = game
        .put_on_battlefield(PlayerId(0), "RAV-SURVEILLING-SPRITE")
        .expect("Sprite setup");
    let drawn = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Library)
        .expect("draw setup");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("lethal spell setup");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
                .expect("red source")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    game.pass_priority(PlayerId(0))
        .expect("active player passes to Char caster");
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(1), mountain, Color::Red)
            .expect("Char mana");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(sprite)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char targets Sprite");
    game.pass_priority(PlayerId(1)).expect("caster passes");
    game.pass_priority(PlayerId(0))
        .expect("Char resolves and Sprite trigger stacks");
    game.pass_priority(PlayerId(0))
        .expect("active player passes trigger");
    game.pass_priority(PlayerId(1))
        .expect("Sprite trigger resolves");

    println!(
        "surveilling_sprite_event_log={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(sprite), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == sprite && *ability == "dies-draw-controller"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == sprite && *ability == "dies-draw-controller"
    )));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-SURVEILLING-SPRITE"));
    game.validate_invariants()
        .expect("dies trigger trace preserves stack invariants");
}
