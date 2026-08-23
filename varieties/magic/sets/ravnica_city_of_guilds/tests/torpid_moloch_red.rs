use cardbench_magic_engine::{AbilityActivation, Game, GameEvent, Keyword, PlayerId, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn torpid_moloch_declares_its_three_land_defender_cost() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TORPID-MOLOCH")
        .expect("Torpid Moloch exists");
    assert!(
        definition
            .supported_rules
            .contains(&"sacrifice-three-lands-remove-defender")
    );
}

#[test]
fn torpid_moloch_sacrifices_three_lands_and_temporarily_loses_defender() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let moloch = game
        .put_on_battlefield(PlayerId(0), "RAV-TORPID-MOLOCH")
        .expect("Moloch enters");
    let lands = [
        game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
            .expect("land one"),
        game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
            .expect("land two"),
        game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
            .expect("land three"),
    ];
    game.begin_game().expect("game starts");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: moloch,
            ability_id: "sacrifice-three-lands-remove-defender",
            sacrifice_sources: lands.to_vec(),
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("land sacrifice activation");
    for land in lands {
        assert_eq!(game.zone_of(land), Some(Zone::Graveyard));
    }
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("ability resolves");
    assert!(
        !game
            .characteristics(moloch)
            .expect("Moloch characteristics")
            .keywords
            .contains(&Keyword::Defender)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { target, .. } if *target == moloch
    )));
}
