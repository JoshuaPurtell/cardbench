//! Red milestone for Viashino Slasher's discard-as-cost pump.

use cardbench_magic_engine::{AbilityActivation, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn viashino_slasher_cannot_activate_without_discarding_a_card() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-VIASHINO-SLASHER")
        .expect("Viashino Slasher enters");
    game.set_entered_turn_for_setup(source, 0)
        .expect("old fixture entry");
    game.add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("discardable card enters hand");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("red mana");

    let result = game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "pump-plus-one-minus-one",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    );
    assert!(
        result.is_err(),
        "the discard-as-cost ability must reject an activation with no discarded card"
    );
}

#[test]
fn viashino_slasher_pump_discards_selected_hand_card_before_stack_activation() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-VIASHINO-SLASHER")
        .expect("Viashino Slasher enters");
    game.set_entered_turn_for_setup(source, 0)
        .expect("old fixture entry");
    let discarded = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("discardable card enters hand");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("red mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "pump-plus-one-minus-one",
            sacrifice_sources: vec![],
            discard_cards: vec![discarded],
            targets: vec![],
        },
    )
    .expect("discard cost is paid");
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            cardbench_magic_engine::GameEvent::DiscardedAsAbilityCost {
                player: PlayerId(0),
                source: event_source,
                card,
            } if *event_source == source && *card == discarded
        )
    }));
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("pump resolves");
    assert_eq!(
        game.characteristics(source).expect("live source").power,
        Some(2)
    );
}
