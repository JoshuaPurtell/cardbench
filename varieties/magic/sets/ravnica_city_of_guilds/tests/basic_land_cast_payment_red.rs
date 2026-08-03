//! Red regression for typed RAV basic lands during spell-cost payment.
//!
//! This uses only public RAV identifiers and rules-state facts. It does not
//! retain card prose, art, or external card-database payloads.

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CastPaymentManaAbility, CastRequest, Color, Game, GameEvent,
    PlayerId, RulesError, Zone,
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
    assert!(game.object(forest).expect("forest exists").tapped);
    assert!(game.object(plains).expect("plains exists").tapped);
    assert_eq!(game.zone_of(spell), None, "the spell is on the stack");
    assert_eq!(game.stack.len(), 1, "only the spell entered the stack");
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .total_exact(),
        0,
        "the final spell payment consumes both intrinsic outputs"
    );
    assert_eq!(
        game.event_log,
        vec![
            GameEvent::CastPaymentBasicLandManaAbilityActivated {
                player,
                card: spell,
                land: forest,
                color: Color::Green,
            },
            GameEvent::ManaAbilityActivated {
                player,
                land: forest,
                color: Color::Green,
            },
            GameEvent::ManaAdded {
                player,
                color: Color::Green,
                amount: 1,
            },
            GameEvent::CastPaymentBasicLandManaAbilityActivated {
                player,
                card: spell,
                land: plains,
                color: Color::White,
            },
            GameEvent::ManaAbilityActivated {
                player,
                land: plains,
                color: Color::White,
            },
            GameEvent::ManaAdded {
                player,
                color: Color::White,
                amount: 1,
            },
            GameEvent::SpellCast {
                player,
                card: spell,
            },
            GameEvent::ObjectIncarnationAdvanced {
                object: spell,
                incarnation: 2,
            },
        ],
        "each typed payment receipt immediately precedes its intrinsic activation and output"
    );
    game.validate_invariants()
        .expect("the typed payment and stack transition preserve invariants");
}

#[test]
fn failed_final_spell_payment_rolls_back_the_typed_basic_land_activation_and_receipts() {
    let player = PlayerId(0);
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV typed basic-land bindings initialize");
    let forest = game
        .put_on_battlefield(player, "RAV-FOREST")
        .expect("the green typed basic land enters the battlefield");
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
                payment_mana_abilities: vec![CastPaymentManaAbility::BasicLand(
                    BasicLandManaAbilityActivation {
                        land: forest,
                        color: Color::Green,
                    },
                )],
            },
        ),
        Err(RulesError::Mana("missing White mana".to_owned()))
    );
    assert!(!game.object(forest).expect("forest exists").tapped);
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .total_exact(),
        0
    );
    assert!(
        game.event_log.is_empty(),
        "the enclosing failed cast restores every typed activation receipt"
    );
    game.validate_invariants()
        .expect("a rejected typed payment leaves the original game shape intact");
}
