//! Event-log coverage for Flame Fusillade's temporary creature ability grant.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

const GRANTED_ABILITY: &str = "granted-tap-deal-one-to-player-or-creature";

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

fn advance_through_turn_one_cleanup(game: &mut Game) {
    while game.turn == 1 {
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.active_player)
                    .expect("attacker view")
                    .attackers_declared =>
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attacker declaration advances combat");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("blocker view")
                    .blockers_declared =>
            {
                game.declare_blockers(game.next_policy_player(), &[])
                    .expect("empty blocker declaration advances combat");
            }
            _ => {
                let first = game.priority;
                game.pass_priority(first).expect("first priority pass");
                if game.turn == 1 && game.step != Step::DeclareAttackers {
                    let second = game.priority;
                    game.pass_priority(second).expect("second priority pass");
                }
            }
        }
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

fn activation(source: cardbench_magic_engine::ObjectId, target: Target) -> AbilityActivation {
    AbilityActivation {
        source,
        ability_id: GRANTED_ABILITY,
        sacrifice_sources: vec![],
        additional_tap_creatures: vec![],
        discard_cards: vec![],
        targets: vec![target],
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The complete layer/activation lifetime is the contract.
fn flame_fusillade_grants_only_current_controller_creatures_and_revokes_at_cleanup() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = rav_game();
    let recipient = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("controller creature setup");
    let nonrecipient = game
        .put_on_battlefield(opponent, "RAV-WATCHWOLF")
        .expect("opponent creature setup");
    for creature in [recipient, nonrecipient] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("creature is not summoning sick");
    }
    let fusillade = game
        .add_card(controller, "RAV-FLAME-FUSILLADE", Zone::Hand)
        .expect("Flame Fusillade setup");
    let mountains = (0..4)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-MOUNTAIN")
                .expect("Flame Fusillade payment land setup")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    for mountain in mountains {
        game.activate_mana_ability(controller, mountain, Color::Red)
            .expect("red mana payment");
    }
    game.cast_spell(
        controller,
        CastRequest {
            card: fusillade,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Flame Fusillade casts");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(fusillade), Some(Zone::Graveyard));
    game.activate_ability(controller, activation(recipient, Target::Player(opponent)))
        .expect("current controller creature receives the normal activation");
    assert!(game.object(recipient).expect("recipient exists").tapped);
    resolve_top(&mut game);
    assert_eq!(game.player(opponent).expect("opponent exists").life, 19);

    game.pass_priority(controller)
        .expect("controller passes to the opponent");
    let before_rejection = game.event_log.clone();
    assert!(
        game.activate_ability(
            opponent,
            activation(nonrecipient, Target::Player(controller))
        )
        .is_err(),
        "an opponent creature was not in the controller's resolution snapshot",
    );
    assert_eq!(game.event_log, before_rejection, "rejection is atomic");

    advance_through_turn_one_cleanup(&mut game);
    for _ in 0..2 {
        if game.priority == controller {
            break;
        }
        let holder = game.priority;
        game.pass_priority(holder)
            .expect("turn state advances to controller priority");
    }
    assert_eq!(game.priority, controller);
    let before_expiry_rejection = game.event_log.clone();
    assert!(
        game.activate_ability(controller, activation(recipient, Target::Player(opponent)))
            .is_err(),
        "the temporary grant must be absent after cleanup",
    );
    assert_eq!(
        game.event_log, before_expiry_rejection,
        "expiry rejection is atomic"
    );

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-FLAME-FUSILLADE"));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated {
            source,
            target,
            layer: cardbench_magic_engine::Layer::Ability,
        } if *source == fusillade && *target == recipient
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { target, .. } if *target == nonrecipient
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityActivated {
            source,
            definition: "RAV-FLAME-FUSILLADE",
            ability: GRANTED_ABILITY,
            ..
        } if *source == recipient
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired {
            source,
            target,
            layer: cardbench_magic_engine::Layer::Ability,
        } if *source == fusillade && *target == recipient
    )));
    println!(
        "flame_fusillade_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("temporary granted ability lifecycle preserves state-machine invariants");
}

#[test]
fn flame_fusillade_activation_resolves_after_its_recipient_leaves_in_response() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = rav_game();
    let recipient = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("controller creature setup");
    game.set_entered_turn_for_setup(recipient, 0)
        .expect("recipient is not summoning sick");
    let fusillade = game
        .add_card(controller, "RAV-FLAME-FUSILLADE", Zone::Hand)
        .expect("Flame Fusillade setup");
    let answer = game
        .add_card(opponent, "RAV-LAST-GASP", Zone::Hand)
        .expect("response setup");
    let mountains = (0..4)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-MOUNTAIN")
                .expect("Flame Fusillade payment land setup")
        })
        .collect::<Vec<_>>();
    let swamp = game
        .put_on_battlefield(opponent, "RAV-SWAMP")
        .expect("response payment land setup");
    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    for mountain in mountains {
        game.activate_mana_ability(controller, mountain, Color::Red)
            .expect("red mana payment");
    }
    game.cast_spell(
        controller,
        CastRequest {
            card: fusillade,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Flame Fusillade casts");
    resolve_top(&mut game);

    game.activate_ability(controller, activation(recipient, Target::Player(opponent)))
        .expect("recipient activation is legal while its grant is live");
    game.pass_priority(controller)
        .expect("controller gives the opponent a response window");
    game.activate_mana_ability(opponent, swamp, Color::Black)
        .expect("response payment mana");
    game.cast_spell(
        opponent,
        CastRequest {
            card: answer,
            targets: vec![Target::Permanent(recipient)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent removes the activation source in response");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(recipient), Some(Zone::Graveyard));
    resolve_top(&mut game);

    assert_eq!(game.player(opponent).expect("opponent exists").life, 19);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == recipient
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == recipient && *ability == GRANTED_ABILITY
    )));
    println!(
        "flame_fusillade_response_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("a departed grant recipient does not invalidate its stacked ability");
}
