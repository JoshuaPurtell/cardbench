//! Event-log contract for Nullstone Gargoyle's turn-scoped cast trigger.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone};
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

fn advance_without_attackers_until(game: &mut Game, target_turn: u32, target_player: PlayerId) {
    for _ in 0..160 {
        if game.turn == target_turn
            && game.active_player == target_player
            && game.step == Step::PrecombatMain
        {
            return;
        }
        if game.step == Step::Draw && !(game.turn == 1 && game.active_player == PlayerId(0)) {
            let player = game.active_player;
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw resolves");
            game.pass_priority(player)
                .expect("drawer passes after its draw");
            let opponent = game.priority;
            game.pass_priority(opponent)
                .expect("opponent advances after draw");
        } else if game.step == Step::DeclareAttackers {
            let player = game.active_player;
            game.declare_attackers(player, &[])
                .expect("empty attack declaration");
            game.pass_priority(player)
                .expect("attacker passes after declaration");
            let opponent = game.priority;
            game.pass_priority(opponent)
                .expect("opponent advances empty combat");
        } else {
            let player = game.priority;
            game.pass_priority(player)
                .expect("priority pass advances turn state");
        }
    }
    panic!("did not reach requested precombat main phase");
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

#[test]
fn first_noncreature_spell_slot_resets_only_at_the_next_untap_turn_boundary() {
    let mut game = game_with_rav_triggers();
    let gargoyle = game
        .put_on_battlefield(PlayerId(0), "RAV-NULLSTONE-GARGOYLE")
        .expect("Gargoyle setup");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("mana source setup");
    let first = game
        .add_card(PlayerId(0), "RAV-TERRARION", Zone::Hand)
        .expect("first artifact setup");
    let second = game
        .add_card(PlayerId(0), "RAV-TERRARION", Zone::Hand)
        .expect("second artifact setup");
    game.add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("player-zero draw filler");
    game.add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Library)
        .expect("player-one draw filler");

    game.begin_game().expect("real turn machine begins");
    advance_without_attackers_until(&mut game, 1, PlayerId(0));
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("turn-one Forest activation");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: first,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("turn-one first spell casts");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(first), Some(Zone::Graveyard));

    advance_without_attackers_until(&mut game, 3, PlayerId(0));
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("new-turn Forest activation after Untap");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: second,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("next-turn first spell casts");
    assert_eq!(
        game.stack.len(),
        2,
        "new turn restores the first-spell trigger"
    );
    resolve_top(&mut game);
    assert_eq!(game.zone_of(second), Some(Zone::Graveyard));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::FirstNoncreatureSpellCastThisTurn {
                    player: PlayerId(0),
                    ..
                }
            ))
            .count(),
        2,
        "turn-one and turn-three casts retain distinct provenance"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCountered { card, source } if *card == second && *source == gargoyle
    )));
    game.validate_invariants()
        .expect("turn boundary reset preserves all state-machine invariants");
}
