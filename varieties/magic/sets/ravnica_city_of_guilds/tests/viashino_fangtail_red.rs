use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn viashino_fangtail_is_executable_with_its_tap_damage_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VIASHINO-FANGTAIL")
        .expect("Viashino Fangtail exists");
    assert_eq!(definition.name, "Viashino Fangtail");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(3), Some(3)));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-deal-one-to-player-or-creature")
    );
}

#[test]
fn viashino_fangtail_taps_and_deals_one_damage_to_a_player() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let fangtail = game
        .put_on_battlefield(PlayerId(0), "RAV-VIASHINO-FANGTAIL")
        .expect("Fangtail enters");
    game.set_entered_turn_for_setup(fangtail, 0)
        .expect("fixture makes Fangtail long-controlled");
    game.begin_game().expect("game starts");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: fangtail,
            ability_id: "tap-deal-one-to-player-or-creature",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Fangtail ability activates");
    assert!(game.object(fangtail).expect("Fangtail remains").tapped);
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("ability resolves");
    assert_eq!(game.player(PlayerId(1)).expect("player exists").life, 19);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer {
            source,
            player: PlayerId(1),
            amount: 1,
        } if *source == fangtail
    )));
    assert_eq!(game.zone_of(fangtail), Some(Zone::Battlefield));
}
