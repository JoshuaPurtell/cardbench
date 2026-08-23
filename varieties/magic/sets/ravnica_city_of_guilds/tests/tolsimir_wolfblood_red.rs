//! Red discovery contract for Tolsimir Wolfblood's two color-specific anthems
//! and named legendary Wolf token ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CreatureSubtype, Game, GameEvent, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings,
};

fn game_with_tolsimir_bindings() -> Game {
    Game::new_with_all_bindings_and_static_continuous_effects(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_static_continuous_effect_bindings(),
    )
    .expect("RAV Tolsimir fixture builds")
}

#[test]
fn tolsimir_wolfblood_requires_color_specific_anthems_and_voja_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TOLSIMIR-WOLFBLOOD")
        .expect("Tolsimir Wolfblood definition exists");

    assert_eq!(definition.name, "Tolsimir Wolfblood");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Green, Color::White])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(3), Some(4)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"other-green-and-white-creatures-get-plus-one-plus-one")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"tap-create-named-legendary-green-white-wolf-token")
    );
}

#[test]
#[allow(clippy::too_many_lines)] // One public trace validates anthem layering and token stack lifecycle.
fn tolsimir_applies_both_live_color_anthems_and_creates_legendary_voja_on_stack() {
    let mut game = game_with_tolsimir_bindings();
    let tolsimir = game
        .add_card(PlayerId(0), "RAV-TOLSIMIR-WOLFBLOOD", Zone::Battlefield)
        .expect("Tolsimir setup");
    let green_white = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("green/white ally setup");
    let green_only = game
        .add_card(PlayerId(0), "RAV-GOLGARI-BROWNSCALE", Zone::Battlefield)
        .expect("green ally setup");
    let off_color = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Battlefield)
        .expect("off-color ally setup");
    let opposing_green_white = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("opposing creature setup");
    game.set_entered_turn_for_setup(tolsimir, 0)
        .expect("Tolsimir may pay a tap cost after setup");

    assert_eq!(
        (
            game.characteristics(tolsimir)
                .expect("Tolsimir remains live")
                .power,
            game.characteristics(tolsimir)
                .expect("Tolsimir remains live")
                .toughness,
        ),
        (Some(3), Some(4)),
        "an other-creature anthem excludes its source"
    );
    assert_eq!(
        (
            game.characteristics(green_white)
                .expect("multicolored ally remains live")
                .power,
            game.characteristics(green_white)
                .expect("multicolored ally remains live")
                .toughness,
        ),
        (Some(5), Some(5)),
        "a green/white creature receives each independent anthem"
    );
    assert_eq!(
        (
            game.characteristics(green_only)
                .expect("green ally remains live")
                .power,
            game.characteristics(green_only)
                .expect("green ally remains live")
                .toughness,
        ),
        (Some(3), Some(4)),
        "a green-only creature receives only the green anthem"
    );
    assert_eq!(
        game.characteristics(off_color)
            .expect("off-color ally remains live")
            .power,
        Some(6),
        "an off-color creature receives no anthem"
    );
    assert_eq!(
        game.characteristics(opposing_green_white)
            .expect("opposing creature remains live")
            .power,
        Some(3),
        "controller-relative anthems do not cross seats"
    );

    game.begin_game().expect("game begins");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: tolsimir,
            ability_id: "tap-create-voja",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Tolsimir activation stacks");
    let first = game.priority;
    game.pass_priority(first).expect("controller passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("opponent resolves activation");

    let voja = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } => Some(*token),
            _ => None,
        })
        .expect("activation creates Voja through the ordinary token receipt");
    let token = game
        .object(voja)
        .expect("created token remains addressable")
        .token
        .as_ref()
        .expect("created object retains its token copiable values");
    assert_eq!(token.name, "Voja");
    assert!(token.is_legendary);
    assert_eq!(token.colors, BTreeSet::from([Color::Green, Color::White]));
    assert_eq!(
        token.creature_subtypes,
        BTreeSet::from([CreatureSubtype::Wolf])
    );
    assert_eq!((token.power, token.toughness), (2, 2));
    assert_eq!(
        (
            game.characteristics(voja).expect("Voja remains live").power,
            game.characteristics(voja)
                .expect("Voja remains live")
                .toughness,
        ),
        (Some(4), Some(4)),
        "Voja receives both live color anthems"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == tolsimir && *ability == "tap-create-voja"
    )));
    eprintln!("Tolsimir trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Tolsimir static and token lifecycles preserve invariants");
}
