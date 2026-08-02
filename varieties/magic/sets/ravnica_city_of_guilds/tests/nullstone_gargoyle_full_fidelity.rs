//! Event-log contract for Nullstone Gargoyle's turn-scoped cast trigger.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_triggers() -> Game {
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

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn first_noncreature_spell_is_countered_once_per_player_per_turn() {
    let mut game = game_with_rav_triggers();
    let gargoyle = game
        .put_on_battlefield(PlayerId(0), "RAV-NULLSTONE-GARGOYLE")
        .expect("Gargoyle setup");
    let first_zero = game
        .add_card(PlayerId(0), "RAV-TERRARION", Zone::Hand)
        .expect("first player-zero spell");
    let second_zero = game
        .add_card(PlayerId(0), "RAV-TERRARION", Zone::Hand)
        .expect("second player-zero spell");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("instant target setup");
    let first_one = game
        .add_card(PlayerId(1), "RAV-DIZZY-SPELL", Zone::Hand)
        .expect("first player-one spell");
    let second_one = game
        .add_card(PlayerId(1), "RAV-DIZZY-SPELL", Zone::Hand)
        .expect("second player-one spell");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("player-zero fixture mana");
    game.grant_mana(PlayerId(1), Color::Blue, 2)
        .expect("player-one fixture mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: first_zero,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("first player-zero noncreature spell casts");
    assert_eq!(game.stack.len(), 2, "Gargoyle trigger is above first spell");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(first_zero), Some(Zone::Graveyard));

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: second_zero,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("second player-zero noncreature spell casts");
    assert_eq!(
        game.stack.len(),
        1,
        "second player-zero spell does not trigger"
    );
    resolve_top(&mut game);
    assert_eq!(game.zone_of(second_zero), Some(Zone::Battlefield));

    game.pass_priority(PlayerId(0))
        .expect("active player yields instant window");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: first_one,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("first player-one noncreature instant casts");
    assert_eq!(
        game.stack.len(),
        2,
        "each player's first cast has its own slot"
    );
    resolve_top(&mut game);
    assert_eq!(game.zone_of(first_one), Some(Zone::Graveyard));

    game.pass_priority(PlayerId(0))
        .expect("active player yields second instant window");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: second_one,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("second player-one noncreature instant casts");
    assert_eq!(
        game.stack.len(),
        1,
        "second player-one spell does not trigger"
    );
    resolve_top(&mut game);
    assert_eq!(game.zone_of(second_one), Some(Zone::Graveyard));

    let first_casts = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::FirstNoncreatureSpellCastThisTurn { turn: 1, .. }
            )
        })
        .count();
    assert_eq!(first_casts, 2, "one first-cast receipt for each player");
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::SpellCountered { source, .. } if *source == gargoyle))
            .count(),
        2,
        "countered first spell still consumes only its caster's turn slot"
    );

    println!("Nullstone Gargoyle trace: {:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("first-spell receipts and trigger stack remain coherent");
}
