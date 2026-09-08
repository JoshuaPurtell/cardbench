//! Red discovery contract for Flash Conscription's temporary control slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn flash_conscription_requires_temporary_control_untap_and_haste() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FLASH-CONSCRIPTION")
        .expect("Flash Conscription definition exists");

    assert_eq!(definition.name, "Flash Conscription");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(5, [Color::Red]));
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"gain-control-until-eot")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"untap-target-permanent")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"grant-haste-until-eot")
    );
}

fn flash_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV Flash Conscription fixture builds")
}

#[test]
fn flash_conscription_changes_control_untaps_and_grants_haste() {
    let mut game = flash_game();
    let spell = game
        .add_card(PlayerId(0), "RAV-FLASH-CONSCRIPTION", Zone::Hand)
        .expect("Flash Conscription enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature enters battlefield");
    game.set_tapped_for_setup(target, true)
        .expect("target creature is tapped for setup");
    game.grant_mana(PlayerId(0), Color::Red, 6)
        .expect("spell mana is available");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Flash Conscription casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes priority");
    game.pass_priority(PlayerId(1))
        .expect("Flash Conscription resolves");

    assert_eq!(
        game.controller_of(target).expect("target controller"),
        PlayerId(0)
    );
    assert!(!game.object(target).expect("target object").tapped);
    assert!(
        game.characteristics(target)
            .expect("target characteristics")
            .keywords
            .contains(&cardbench_magic_engine::Keyword::Haste)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentUntapped { source, card } if *source == spell && *card == target
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::ContinuousEffectCreated { source, target: effect_target, .. } if *source == spell && *effect_target == target))
            .count(),
        3
    );
    game.validate_invariants()
        .expect("Flash Conscription trace is invariant-valid");
}

#[test]
fn flash_conscription_uses_one_target_occurrence_for_its_ordered_bundle() {
    let mut game = flash_game();
    let spell = game
        .add_card(PlayerId(0), "RAV-FLASH-CONSCRIPTION", Zone::Hand)
        .expect("Flash Conscription enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature enters battlefield");
    game.set_tapped_for_setup(target, true)
        .expect("target creature is tapped for setup");
    game.grant_mana(PlayerId(0), Color::Red, 6)
        .expect("spell mana is available");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            // One printed target must be retained and independently rechecked
            // for each part of the ordered target-effect bundle.
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("one printed target casts the complete Flash Conscription bundle");
}
