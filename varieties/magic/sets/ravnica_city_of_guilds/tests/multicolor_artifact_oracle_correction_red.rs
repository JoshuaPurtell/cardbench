//! Red regression probes for the Cyclopean Snare and Grifter's Blade facts
//! that an earlier catalogue promotion misstated.
//!
//! These tests deliberately use normal stack actions.  They are a guard
//! against calling a card "full fidelity" because a superficially similar
//! activation or Equipment substrate happens to exist.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Effect, Game, Keyword, ManaCost, PlayerId, PolicyAction,
    Step, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

fn game() -> Game {
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
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register before play");
    game
}

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != Step::PrecombatMain {
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first response pass");
    let second = game.priority;
    game.pass_priority(second).expect("second response pass");
}

#[test]
fn cyclopean_snare_has_its_printed_two_mana_cast_cost() {
    assert_eq!(
        definition("RAV-CYCLOPEAN-SNARE").mana_cost,
        ManaCost::new(2),
        "Cyclopean Snare costs {{2}}, not {{3}}"
    );
}

#[test]
fn cyclopean_snare_has_its_printed_three_mana_tap_activation() {
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-CYCLOPEAN-SNARE"
                && binding.ability.id == "tap-target-creature"
        })
        .expect("Cyclopean Snare activation exists");
    assert_eq!(
        binding.ability.mana_cost,
        ManaCost::new(3),
        "Cyclopean Snare activates for {{3}}, {{T}}"
    );
    assert_eq!(
        binding.ability.effects,
        vec![Effect::TapTargetCreature, Effect::ReturnSourceToOwnersHand,],
        "the activation must tap its target and return the Snare at resolution"
    );
}

#[test]
fn grifters_blade_has_its_printed_cost_and_flash() {
    let blade = definition("RAV-GRIFTERS-BLADE");
    assert_eq!(
        blade.mana_cost,
        ManaCost::new(3),
        "Grifter's Blade costs {{3}}"
    );
    assert_eq!(
        blade.keywords,
        vec![Keyword::Flash],
        "Grifter's Blade has Flash"
    );
}

#[test]
fn cyclopean_snare_returns_itself_only_after_its_stack_ability_resolves() {
    let mut game = game();
    let snare = game
        .put_on_battlefield(PlayerId(0), "RAV-CYCLOPEAN-SNARE")
        .expect("Snare enters before the measured game");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature enters");
    let plains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("activation mana enters before the measured game")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    advance_to_precombat_main(&mut game);
    for land in plains {
        game.activate_mana_ability(PlayerId(0), land, Color::White)
            .expect("activation mana is available");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: snare,
            ability_id: "tap-target-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Cyclopean Snare activation is legal");
    assert_eq!(game.zone_of(snare), Some(Zone::Battlefield));
    resolve_top(&mut game);
    println!(
        "Cyclopean Snare correction trace: {:#?}",
        game.canonical_event_log()
    );
    assert!(game.object(target).expect("target exists").tapped);
    assert_eq!(
        game.zone_of(snare),
        Some(Zone::Hand),
        "Cyclopean Snare returns to its owner's hand at resolution"
    );
    game.validate_invariants()
        .expect("self-return ability leaves an auditable state");
}

#[test]
fn grifters_blade_flashes_in_then_its_entry_trigger_attaches_when_a_target_exists() {
    let mut game = game();
    let blade = game
        .add_card(PlayerId(1), "RAV-GRIFTERS-BLADE", Zone::Hand)
        .expect("Blade enters hand before the measured game");
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("controlled creature enters before the measured game");
    let plains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-PLAINS")
                .expect("cast mana enters before the measured game")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    game.pass_priority(PlayerId(0))
        .expect("nonactive player receives priority in upkeep");
    for land in plains {
        game.activate_mana_ability(PlayerId(1), land, Color::White)
            .expect("cast mana is available");
    }
    let cast = game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: blade,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    println!(
        "Grifter's Blade flash-cast pre-resolution trace: {:#?}",
        game.canonical_event_log()
    );
    cast.expect("Flash permits Grifter's Blade to be cast during the opponent's upkeep");
    resolve_top(&mut game);
    game.submit_policy_move(
        PlayerId(1),
        "test.grifters-blade-entry-attachment.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            decision: game
                .view_for_player(PlayerId(1))
                .expect("controller view")
                .triggered_ability_target_choice
                .expect("entry target choice")
                .decision,
            source: blade,
            ability: "etb-attach-to-controlled-creature",
            targets: vec![Target::Permanent(creature)],
        },
    )
    .expect("entry trigger asks its controller for one controlled creature");
    resolve_top(&mut game);
    println!(
        "Grifter's Blade correction trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(blade), Some(Zone::Battlefield));
    assert_eq!(
        game.object(blade).expect("Blade exists").attached_to,
        Some(creature)
    );
    let characteristics = game
        .characteristics(creature)
        .expect("creature characteristics");
    assert_eq!(characteristics.power, Some(4));
    assert_eq!(characteristics.toughness, Some(4));
    game.validate_invariants()
        .expect("Flash entry attachment leaves an auditable state");
}

#[test]
fn cyclopean_snare_never_returns_a_source_that_left_before_resolution() {
    let mut game = game();
    let snare = game
        .put_on_battlefield(PlayerId(0), "RAV-CYCLOPEAN-SNARE")
        .expect("Snare enters before the measured game");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("tap target enters before the measured game");
    let activation_plains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("activation mana enters before the measured game")
        })
        .collect::<Vec<_>>();
    let smash = game
        .add_card(PlayerId(1), "RAV-SMASH", Zone::Hand)
        .expect("Smash enters hand before the measured game");
    game.add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("Smash draw has one library card available");
    let response_lands = [
        game.put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
            .expect("red response mana enters"),
        game.put_on_battlefield(PlayerId(1), "RAV-PLAINS")
            .expect("generic response mana enters"),
        game.put_on_battlefield(PlayerId(1), "RAV-PLAINS").unwrap(),
    ];
    game.begin_game().expect("game starts");
    advance_to_precombat_main(&mut game);
    for land in activation_plains {
        game.activate_mana_ability(PlayerId(0), land, Color::White)
            .expect("Snare activation mana is available");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: snare,
            ability_id: "tap-target-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Snare ability starts on the stack");
    game.pass_priority(PlayerId(0))
        .expect("opponent receives response priority");
    game.activate_mana_ability(PlayerId(1), response_lands[0], Color::Red)
        .expect("red response mana is available");
    game.activate_mana_ability(PlayerId(1), response_lands[1], Color::White)
        .expect("generic response mana is available");
    game.activate_mana_ability(PlayerId(1), response_lands[2], Color::White).unwrap();
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: smash,
            targets: vec![Target::Permanent(snare)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Smash can destroy the Snare in response");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(snare), Some(Zone::Graveyard));
    resolve_top(&mut game);
    println!(
        "Cyclopean Snare departed-source trace: {:#?}",
        game.canonical_event_log()
    );
    assert!(game.object(target).expect("tap target exists").tapped);
    assert_eq!(
        game.zone_of(snare),
        Some(Zone::Graveyard),
        "a departed source's old ability must not return a later incarnation"
    );
    game.validate_invariants()
        .expect("source-return provenance remains auditable after a response");
}

#[test]
fn grifters_blade_enters_unattached_when_no_controlled_creature_is_legal() {
    let mut game = game();
    let blade = game
        .add_card(PlayerId(1), "RAV-GRIFTERS-BLADE", Zone::Hand)
        .expect("Blade enters hand before the measured game");
    let plains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-PLAINS")
                .expect("cast mana enters before the measured game")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    game.pass_priority(PlayerId(0))
        .expect("nonactive player receives priority in upkeep");
    for land in plains {
        game.activate_mana_ability(PlayerId(1), land, Color::White)
            .expect("cast mana is available");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: blade,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Flash permits the no-target entry cast");
    resolve_top(&mut game);
    println!(
        "Grifter's Blade no-target entry trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(blade), Some(Zone::Battlefield));
    assert_eq!(game.object(blade).expect("Blade exists").attached_to, None);
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        cardbench_magic_engine::GameEvent::EquipmentAttached { equipment, .. } if *equipment == blade
    )));
    game.validate_invariants()
        .expect("unattached Equipment entry remains a valid state");
}
