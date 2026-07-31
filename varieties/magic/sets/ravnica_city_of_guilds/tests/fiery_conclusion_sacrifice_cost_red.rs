//! Red regression: Fiery Conclusion must not be cast without its creature sacrifice cost.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, RulesError, Target, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn fiery_conclusion_rejects_a_cast_without_a_controlled_creature_sacrifice() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    let conclusion = game
        .add_card(PlayerId(0), "RAV-FIERY-CONCLUSION", Zone::Hand)
        .expect("Fiery Conclusion enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("opponent target enters battlefield");
    game.grant_mana(PlayerId(0), Color::Red, 2)
        .expect("fixture mana");
    game.clear_event_log();

    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: conclusion,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );

    println!("Fiery Conclusion missing-sacrifice result: {result:?}");
    println!(
        "Fiery Conclusion missing-sacrifice events: {:?}",
        game.event_log
    );
    assert!(
        matches!(result, Err(RulesError::IllegalAction(_))),
        "Fiery Conclusion must fail closed until its required creature sacrifice is supplied"
    );
    assert_eq!(game.zone_of(conclusion), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
}
