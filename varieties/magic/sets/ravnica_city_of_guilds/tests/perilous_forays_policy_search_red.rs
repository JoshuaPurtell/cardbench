//! Red regression for Perilous Forays' controller-private library choice.

use cardbench_magic_engine::{AbilityActivation, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn perilous_forays_waits_for_its_controllers_private_basic_land_choice() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let forays = game
        .add_card(PlayerId(0), "RAV-PERILOUS-FORAYS", Zone::Battlefield)
        .expect("Perilous Forays setup");
    let victim = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("sacrifice cost setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("Forest setup");
    let plains = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("Plains setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("ability mana");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: forays,
            ability_id: "sacrifice-creature-search-basic-land",
            sacrifice_sources: vec![victim],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Perilous Forays activation");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .pending_decision
            .is_some(),
        "Perilous Forays must suspend for its controller's private library choice; trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(forest), Some(Zone::Library));
    assert_eq!(game.zone_of(plains), Some(Zone::Library));
}
