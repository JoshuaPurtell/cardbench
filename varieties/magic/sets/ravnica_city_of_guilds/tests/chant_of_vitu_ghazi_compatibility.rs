//! Green integration regression for Chant's creature-damage replacement.

use cardbench_magic_engine::{
    CastRequest, Color, ConvokeContribution, ConvokePayment, Game, GameEvent, PlayerId, Step,
    Target, Zone,
};
use cardbench_magic_rav::card_definitions;

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_opponent_declare_attackers(game: &mut Game) {
    while game.active_player != PlayerId(1) || game.step != Step::DeclareAttackers {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty attacker declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty blocker declaration is legal");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
}

fn add_opening_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, "RAV-FOREST", Zone::Library)
            .expect("opening library fixture exists");
    }
}

#[test]
fn chant_convoke_prevents_attack_damage_and_gains_only_that_amount() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    add_opening_library(&mut game, PlayerId(0));
    add_opening_library(&mut game, PlayerId(1));
    let first = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("first Convoke creature enters");
    let second = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-RECRUIT")
        .expect("second Convoke creature enters");
    let attacker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opposing attacker enters");
    for creature in [first, second, attacker] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("creature is long-controlled");
    }
    let chant = game
        .add_card(PlayerId(0), "RAV-CHANT-OF-VITU-GHAZI", Zone::Hand)
        .expect("Chant begins in hand");
    game.begin_game().expect("fixture begins game");
    advance_to_opponent_declare_attackers(&mut game);

    game.declare_attackers(PlayerId(1), &[attacker])
        .expect("opponent attacks");
    game.pass_priority(PlayerId(1))
        .expect("attacker passes for the response");
    game.add_mana_from_action(PlayerId(0), Color::White, 6)
        .expect("six white mana pays the reduced cost");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: chant,
            targets: vec![],
            convoke: vec![
                ConvokePayment {
                    creature: first,
                    contribution: ConvokeContribution::Generic,
                },
                ConvokePayment {
                    creature: second,
                    contribution: ConvokeContribution::Generic,
                },
            ],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Chant casts through Convoke");
    pass_pair(&mut game);
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(PlayerId(0), &[])
        .expect("no blockers declared");
    pass_pair(&mut game);

    assert_eq!(game.players[0].life, 23);
    assert_eq!(game.players[1].life, 20);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CreatureDamagePrevented {
            source,
            prevented_by,
            controller: PlayerId(0),
            target: Target::Player(PlayerId(0)),
            amount: 3,
            ..
        } if *source == attacker && *prevented_by == chant
    )));
    game.validate_invariants()
        .expect("Convoke and damage prevention receipts remain valid");
}
