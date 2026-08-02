//! Event-log coverage for Galvanic Arc's attachment-granted activation.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn rav_game() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game
}

fn advance_to_precombat_main(game: &mut Game) {
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first priority pass");
        let second = game.priority;
        game.pass_priority(second).expect("second priority pass");
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn galvanic_arc_grants_the_enchanted_creature_a_stack_backed_tap_damage_ability() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = rav_game();
    let creature = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("creature setup");
    game.set_entered_turn_for_setup(creature, 0)
        .expect("creature is not summoning sick");
    let arc = game
        .add_card(controller, "RAV-GALVANIC-ARC", Zone::Hand)
        .expect("Aura setup");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-MOUNTAIN")
                .expect("Aura payment land setup")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    for mountain in mountains {
        game.activate_mana_ability(controller, mountain, Color::Red)
            .expect("red mana payment");
    }

    let before_attachment = game.event_log.clone();
    assert!(
        game.activate_ability(
            controller,
            AbilityActivation {
                source: creature,
                ability_id: "attached-tap-deal-three-to-player-or-creature",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Player(opponent)],
            },
        )
        .is_err(),
        "the creature cannot activate a grant before the Aura attaches",
    );
    assert_eq!(game.event_log, before_attachment, "rejection is atomic");

    game.cast_spell(
        controller,
        CastRequest {
            card: arc,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Galvanic Arc casts");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(arc), Some(Zone::Battlefield));
    assert_eq!(
        game.object(arc).expect("Aura exists").attached_to,
        Some(creature)
    );

    game.activate_ability(
        controller,
        AbilityActivation {
            source: creature,
            ability_id: "attached-tap-deal-three-to-player-or-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(opponent)],
        },
    )
    .expect("the live attachment grants its creature the ability");
    assert!(game.object(creature).expect("creature exists").tapped);
    resolve_top(&mut game);

    assert_eq!(game.player(opponent).expect("opponent exists").life, 17);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AttachmentEstablishedWithoutContinuousEffect {
            attachment,
            target,
            ..
        } if *attachment == arc && *target == creature
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityActivated {
            source,
            definition: "RAV-GALVANIC-ARC",
            ability: "attached-tap-deal-three-to-player-or-creature",
            ..
        } if *source == creature
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer {
            source,
            player,
            amount: 3,
        } if *source == creature && *player == opponent
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved {
            source,
            ability: "attached-tap-deal-three-to-player-or-creature",
            ..
        } if *source == creature
    )));
    println!("galvanic_arc_event_log={:#?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("attachment-granted ability trace preserves state-machine invariants");
}

#[test]
fn galvanic_arc_ability_remains_on_the_stack_after_the_aura_is_destroyed_in_response() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = rav_game();
    let creature = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("creature setup");
    game.set_entered_turn_for_setup(creature, 0)
        .expect("creature is not summoning sick");
    let arc = game
        .add_card(controller, "RAV-GALVANIC-ARC", Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(arc, creature)
        .expect("Aura attachment setup");
    let answer = game
        .add_card(opponent, "RAV-LEAVE-NO-TRACE", Zone::Hand)
        .expect("response setup");
    let plains = (0..2)
        .map(|_| {
            game.put_on_battlefield(opponent, "RAV-PLAINS")
                .expect("response payment land setup")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    game.activate_ability(
        controller,
        AbilityActivation {
            source: creature,
            ability_id: "attached-tap-deal-three-to-player-or-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(opponent)],
        },
    )
    .expect("the live Aura grants the creature's activation");
    game.pass_priority(controller)
        .expect("controller gives the opponent a response window");
    for plain in plains {
        game.activate_mana_ability(opponent, plain, Color::White)
            .expect("response payment mana");
    }
    game.cast_spell(
        opponent,
        CastRequest {
            card: answer,
            targets: vec![Target::Permanent(arc)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the opponent destroys the granting Aura in response");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(arc), Some(Zone::Graveyard));
    resolve_top(&mut game);

    assert_eq!(game.player(opponent).expect("opponent exists").life, 17);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { card, .. } if *card == arc
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved {
            source,
            ability: "attached-tap-deal-three-to-player-or-creature",
            ..
        } if *source == creature
    )));
    println!(
        "galvanic_arc_response_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("departed attachment does not invalidate an already-stacked ability");
}
