//! Red regression for explicit generic-mana allocation and Ribbons of Night.
//!
//! This intentionally names only semantic engine data, not card rules prose.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, ManaPaymentSelection, PlayerId, Target, Zone,
};
use cardbench_magic_rav::card_definitions;

#[test]
fn ribbons_can_replay_a_nonblue_generic_payment_without_granting_the_conditional_draw() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV game constructs");
    let ribbons = game
        .add_card(caster, "RAV-RIBBONS-OF-NIGHT", Zone::Hand)
        .expect("Ribbons begins in hand");
    let target = game
        .put_on_battlefield(opponent, "RAV-GOLGARI-BROWNSCALE")
        .expect("target creature begins on the battlefield");
    let drawn = game
        .add_card(caster, "RAV-WATCHWOLF", Zone::Library)
        .expect("public library fixture has one drawable card");
    for (color, amount) in [(Color::Black, 1), (Color::Blue, 4), (Color::Red, 4)] {
        game.grant_mana(caster, color, amount)
            .expect("fixture mana fits the bounded pool");
    }
    game.clear_event_log();

    game.cast_spell_with_mana_spend(
        caster,
        CastRequest {
            card: ribbons,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![Color::Red; 4],
            hybrid: vec![],
        },
    )
    .expect("the player may select Red, rather than available Blue, for the four generic symbols");
    assert_eq!(
        game.event_log,
        vec![
            GameEvent::SpellManaPaid {
                player: caster,
                card: ribbons,
                colors: vec![Color::Black, Color::Red, Color::Red, Color::Red, Color::Red],
            },
            GameEvent::SpellCast {
                player: caster,
                card: ribbons,
            },
        ],
        "the selected allocation must be replayable from the ordered cast receipt"
    );

    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(opponent).expect("opponent passes and resolves");
    assert_eq!(game.zone_of(drawn), Some(Zone::Library));
    assert_eq!(
        game.player(caster).expect("caster exists").life,
        24,
        "the supported damage/life instructions still resolve"
    );
    game.validate_invariants()
        .expect("the paid-color receipt remains attached to the stack lifecycle");
}
