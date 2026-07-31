//! Red regression for explicit generic-mana allocation and Ribbons of Night.
//!
//! This intentionally names only semantic engine data, not card rules prose.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, RulesError, Target, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn ribbons_rejects_an_unselected_generic_payment_instead_of_silently_choosing_blue() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV game constructs");
    let ribbons = game
        .add_card(caster, "RAV-RIBBONS-OF-NIGHT", Zone::Hand)
        .expect("Ribbons begins in hand");
    let target = game
        .put_on_battlefield(opponent, "RAV-GOLGARI-BROWNSCALE")
        .expect("target creature begins on the battlefield");
    for (color, amount) in [(Color::Black, 1), (Color::Blue, 4), (Color::Red, 4)] {
        game.grant_mana(caster, color, amount)
            .expect("fixture mana fits the bounded pool");
    }
    game.clear_event_log();

    let result = game.cast_spell(
        caster,
        CastRequest {
            card: ribbons,
            targets: vec![Target::Permanent(target)],
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
        "four Blue and four Red are both legal generic-payment allocations, but the legacy request cannot select either; the card must fail closed until a typed selection is submitted"
    );
    assert_eq!(game.zone_of(ribbons), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("the rejected ambiguous payment preserves the original state");
}
