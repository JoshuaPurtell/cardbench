//! Red discovery probe for Hammerfist Giant's tap damage ability.

use cardbench_magic_engine::{AbilityActivation, CardType, Game, GameEvent, Keyword, PlayerId};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};
use cardbench_magic_rav::{
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn hammerfist_giant_requires_nonflying_global_damage_and_tap_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HAMMERFIST-GIANT")
        .expect("Hammerfist Giant definition exists");
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-global-nonflying-damage")
    );
    assert_eq!(definition.power, Some(5));
    assert_eq!(definition.toughness, Some(4));
    assert!(!definition.keywords.contains(&Keyword::Flying));
}

#[test]
fn hammerfist_giant_damages_nonfliers_but_not_fliers() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let giant = game
        .put_on_battlefield(PlayerId(0), "RAV-HAMMERFIST-GIANT")
        .expect("Giant enters");
    let ground = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("ground creature enters");
    let flyer = game
        .put_on_battlefield(PlayerId(0), "RAV-COURIER-HAWK")
        .expect("flying creature enters");
    game.set_entered_turn_for_setup(giant, 0)
        .expect("Giant has prior-turn provenance");
    game.set_entered_turn_for_setup(ground, 0)
        .expect("ground creature has prior-turn provenance");
    game.set_entered_turn_for_setup(flyer, 0)
        .expect("flyer has prior-turn provenance");
    game.begin_game().expect("game starts");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player).expect("advance to first main");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: giant,
            ability_id: "tap-global-nonflying-damage",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Giant activation succeeds");
    game.pass_priority(PlayerId(0))
        .expect("Giant controller passes");
    game.pass_priority(PlayerId(1))
        .expect("Giant ability resolves");
    assert_eq!(game.object(giant).expect("Giant survives").damage, 4);
    assert!(
        !game
            .player(PlayerId(0))
            .expect("player exists")
            .battlefield
            .contains(&ground)
    );
    assert!(
        game.player(PlayerId(0))
            .expect("player exists")
            .battlefield
            .contains(&flyer)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount }
            if *source == giant && *permanent == giant && *amount == 4
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { permanent, .. } if *permanent == flyer
    )));
}
