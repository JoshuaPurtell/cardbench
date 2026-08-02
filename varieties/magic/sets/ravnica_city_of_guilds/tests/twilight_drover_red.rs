//! Red discovery contract for Twilight Drover's leave-the-battlefield trigger
//! and flying Spirit activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, Target, Zone,
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
            .contains(&"another-creature-leaves-battlefield-plus-one-counter")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"activated-create-flying-spirit")
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn twilight_drover_triggers_when_another_creature_bounces_and_creates_spirit() {
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
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == drover && *ability == "another-creature-leaves-plus-one-counter"
    )));

    game.pass_priority(PlayerId(0))
        .expect("Drover trigger pass");
    game.pass_priority(PlayerId(1))
        .expect("Drover trigger enters optional choice");
    game.submit_policy_move(
        PlayerId(0),
        "twilight-drover-test.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            source: drover,
            ability: "another-creature-leaves-plus-one-counter",
            pay: true,
            target: None,
        },
    )
    .expect("accept Drover trigger");
    assert_eq!(
        game.object(drover)
            .expect("Drover remains")
            .counters
            .get(&cardbench_magic_engine::CounterKind::PlusOnePlusOne),
        Some(&1)
    );

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: drover,
            ability_id: "create-flying-spirit",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Spirit activation stacks");
    game.pass_priority(PlayerId(0)).expect("activation pass");
    game.pass_priority(PlayerId(1))
        .expect("activation resolves");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)
    )));
    assert_eq!(
        game.players[0]
            .battlefield
            .iter()
            .filter(|card| game
                .object(**card)
                .expect("battlefield object")
                .token
                .is_some())
            .count(),
        1
    );
    assert_eq!(game.zone_of(white), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("Twilight Drover preserves invariants");
    eprintln!("Twilight Drover trace={:?}", game.canonical_event_log());
}
