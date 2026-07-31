//! Red probe for Hunted Horror's targeted opponent token ETB ability.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn hunted_horror_etb_creates_two_target_opponent_tokens() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let horror = game.add_card(PlayerId(0), "RAV-HUNTED-HORROR", Zone::Hand);
    println!(
        "Hunted Horror setup result: {horror:?}; events={:?}",
        game.canonical_event_log()
    );
    let horror = horror.expect("Hunted Horror must be executable");
    game.begin_game().expect("fixture starts game");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward main phase");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward main phase");
    }
    game.grant_mana(PlayerId(0), Color::Black, 2)
        .expect("fixture grants black mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: horror,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Hunted Horror casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the creature spell");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes the creature spell");
    println!("Hunted Horror red trace: {:?}", game.canonical_event_log());
    assert_eq!(game.stack.len(), 1, "ETB ability should be on the stack");
}
