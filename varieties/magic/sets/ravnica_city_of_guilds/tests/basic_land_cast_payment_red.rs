//! Red regression for typed RAV basic lands during spell-cost payment.
//!
//! This uses only public RAV identifiers and rules-state facts. It does not
//! retain card prose, art, or external card-database payloads.

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CastPaymentManaAbility, CastRequest, Color, Game, PlayerId,
    Zone,
};
use cardbench_magic_rav::{card_definitions, rav_basic_land_type_bindings};

#[test]
fn typed_rav_basic_lands_can_pay_a_colored_spell_cost_inside_one_cast() {
    let player = PlayerId(0);
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV typed basic-land bindings initialize");
    let forest = game
        .put_on_battlefield(player, "RAV-FOREST")
        .expect("the green typed basic land enters the battlefield");
    let plains = game
        .put_on_battlefield(player, "RAV-PLAINS")
        .expect("the white typed basic land enters the battlefield");
    let spell = game
        .add_card(player, "RAV-WATCHWOLF", Zone::Hand)
        .expect("the public two-color creature begins in hand");
    game.clear_event_log();

    assert_eq!(
        game.cast_spell(
            player,
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![
                    CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                        land: forest,
                        color: Color::Green,
                    }),
                    CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                        land: plains,
                        color: Color::White,
                    }),
                ],
            },
        ),
        Ok(()),
        "the player explicitly selects the two typed intrinsic mana abilities required to cast the spell"
    );
}
