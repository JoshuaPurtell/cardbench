//! Green regression for Chant of Vitu-Ghazi's dynamic resolution count.

use cardbench_magic_engine::{
    CastRequest, Color, ConvokeContribution, ConvokePayment, Game, GameEvent, PlayerId, Zone,
};
use cardbench_magic_rav::card_definitions;

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn chant_counts_opposing_and_controller_creatures_at_resolution() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    let first = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("first creature enters");
    let second = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-RECRUIT")
        .expect("second creature enters");
    let opposing = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opposing creature enters");
    for creature in [first, second, opposing] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("creature is long-controlled");
    }
    let chant = game
        .add_card(PlayerId(0), "RAV-CHANT-OF-VITU-GHAZI", Zone::Hand)
        .expect("Chant begins in hand");
    game.begin_game().expect("fixture begins game");
    game.add_mana_from_action(PlayerId(0), Color::White, 8)
        .expect("fixture mana is added with priority");
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
    resolve_top(&mut game);

    assert_eq!(game.players[0].life, 23, "three creatures grant three life");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained {
            player: PlayerId(0),
            amount: 3,
        }
    )));
    game.validate_invariants()
        .expect("dynamic life-gain state remains invariant-valid");
}
