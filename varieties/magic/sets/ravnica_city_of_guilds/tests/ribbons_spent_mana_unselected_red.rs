//! Red regression: a paid-color conditional cannot use implicit generic mana.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, RulesError, Target, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn paid_color_conditional_rejects_the_legacy_unselected_cast_path() {
    let caster = PlayerId(0);
    let mut game = Game::new(card_definitions(), 2).expect("RAV game constructs");
    let ribbons = game
        .add_card(caster, "RAV-RIBBONS-OF-NIGHT", Zone::Hand)
        .expect("Ribbons begins in hand");
    let victim = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("target creature begins on the battlefield");
    for (color, amount) in [(Color::Black, 1), (Color::Blue, 4), (Color::Red, 4)] {
        game.grant_mana(caster, color, amount)
            .expect("fixture mana fits the pool");
    }
    game.clear_event_log();

    let result = game.cast_spell(
        caster,
        CastRequest {
            card: ribbons,
            targets: vec![Target::Permanent(victim)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    println!("Ribbons unselected-payment result: {result:?}");
    println!("Ribbons unselected-payment events: {:?}", game.event_log);
    assert_eq!(
        result,
        Err(RulesError::IllegalAction(
            "spell requires an explicit mana-spend selection"
        )),
        "the legacy request cannot select Blue or Red for generic symbols, so it must fail closed"
    );
    assert_eq!(game.zone_of(ribbons), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected ambiguous payment preserves the original state");
}
