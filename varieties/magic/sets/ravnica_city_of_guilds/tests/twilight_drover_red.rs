//! Red discovery contract for Twilight Drover's token-only leave trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, CounterKind, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn twilight_drover_definition_and_rules_contract_exist() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TWILIGHT-DROVER")
        .expect("Twilight Drover definition exists");
    assert_eq!(definition.name, "Twilight Drover");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(1)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"creature-token-leaves-battlefield-plus-one-counter")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"activated-remove-counter-create-two-flying-spirits")
    );
}

#[test]
fn twilight_drover_does_not_trigger_when_a_nontoken_creature_bounces() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let drover = game
        .put_on_battlefield(PlayerId(0), "RAV-TWILIGHT-DROVER")
        .expect("Twilight Drover setup");
    let exiter = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("bounced creature setup");
    let clutch = game
        .add_card(PlayerId(0), "RAV-CLUTCH-OF-THE-UNDERCITY", Zone::Hand)
        .expect("bounce spell setup");
    let white = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Battlefield)
        .expect("white source setup");
    let white_two = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Battlefield)
        .expect("second white source setup");
    let white_three = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Battlefield)
        .expect("third white source setup");
    let blue = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Battlefield)
        .expect("blue source setup");
    let blue_two = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Battlefield)
        .expect("second blue source setup");
    let black_one = game
        .add_card(PlayerId(0), "RAV-SWAMP", Zone::Battlefield)
        .expect("black source setup");
    let black_two = game
        .add_card(PlayerId(0), "RAV-SWAMP", Zone::Battlefield)
        .expect("second black source setup");
    game.begin_game().expect("game begins");
    for (land, color) in [
        (blue, Color::Blue),
        (blue_two, Color::Blue),
        (black_one, Color::Black),
        (black_two, Color::Black),
        (white, Color::White),
        (white_two, Color::White),
        (white_three, Color::White),
    ] {
        game.activate_mana_ability(PlayerId(0), land, color)
            .expect("land produces payment mana");
    }

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: clutch,
            targets: vec![Target::Permanent(exiter)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Clutch casts");
    game.pass_priority(PlayerId(0)).expect("Clutch pass");
    game.pass_priority(PlayerId(1)).expect("Clutch resolves");
    assert_eq!(game.zone_of(exiter), Some(Zone::Hand));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == drover && *ability == "creature-token-leaves-plus-one-counter"
    )));
    assert_eq!(
        game.object(drover)
            .expect("Drover remains")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        None
    );
    assert_eq!(game.zone_of(white), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("Twilight Drover preserves invariants");
}
