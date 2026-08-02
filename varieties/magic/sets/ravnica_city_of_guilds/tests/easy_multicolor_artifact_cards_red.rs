//! Red regression probes for small RAV multicolor/artifact cards.
//!
//! These cards are intentionally exercised through the expansion-neutral
//! trigger, activated-ability, and attachment substrates rather than through
//! card-name branches.

use cardbench_magic_engine::{
    AbilityActivation, Color, Game, GameEvent, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn centaur_safeguard_dies_life_gain_is_full_fidelity() {
    let safeguard = definition("RAV-CENTAUR-SAFEGUARD");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&safeguard.id),
        "Centaur Safeguard must be in the full-fidelity manifest"
    );
}

#[test]
fn cyclopean_snare_tap_activation_is_full_fidelity() {
    let snare = definition("RAV-CYCLOPEAN-SNARE");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&snare.id),
        "Cyclopean Snare must be in the full-fidelity manifest"
    );
}

#[test]
fn grifters_blade_equipment_attachment_is_full_fidelity() {
    let blade = definition("RAV-GRIFTERS-BLADE");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&blade.id),
        "Grifter's Blade must be in the full-fidelity manifest"
    );
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
        .expect("RAV Equipment bindings register before play");
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
fn centaur_safeguard_dies_then_its_controller_can_accept_three_life() {
    let mut game = game();
    let safeguard = game
        .put_on_battlefield(PlayerId(0), "RAV-CENTAUR-SAFEGUARD")
        .expect("Safeguard enters before the measured game");
    let last_gasp = game
        .add_card(PlayerId(1), "RAV-LAST-GASP", Zone::Hand)
        .expect("Last Gasp enters hand");
    let swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("Swamp enters before the measured game");
    game.begin_game().expect("game starts");
    advance_to_precombat_main(&mut game);
    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.activate_mana_ability(PlayerId(1), swamp, Color::Black)
        .expect("opponent produces black mana");
    game.cast_spell(
        PlayerId(1),
        cardbench_magic_engine::CastRequest {
            card: last_gasp,
            targets: vec![Target::Permanent(safeguard)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Last Gasp targets Safeguard");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(safeguard), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == safeguard && *ability == "dies-may-gain-three-life"
    )));

    resolve_top(&mut game);
    game.submit_policy_move(
        PlayerId(0),
        "test.centaur-safeguard-accept.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            source: safeguard,
            ability: "dies-may-gain-three-life",
            pay: true,
            target: None,
        },
    )
    .expect("controller accepts optional life gain");
    println!("Centaur Safeguard trace: {:#?}", game.canonical_event_log());
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 23);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained { player, amount }
            if *player == PlayerId(0) && *amount == 3
    )));
    game.validate_invariants()
        .expect("dies-trigger event sequence is valid");
}

#[test]
fn cyclopean_snare_pays_taps_and_resolves_on_the_stack() {
    let mut game = game();
    let snare = game
        .put_on_battlefield(PlayerId(0), "RAV-CYCLOPEAN-SNARE")
        .expect("Snare enters before the measured game");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature enters");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains enters before the measured game");
    game.begin_game().expect("game starts");
    advance_to_precombat_main(&mut game);
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("generic activation payment is available");
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
    .expect("Snare activation is legal");
    resolve_top(&mut game);
    println!("Cyclopean Snare trace: {:#?}", game.canonical_event_log());
    assert!(game.object(snare).expect("Snare exists").tapped);
    assert!(game.object(target).expect("target exists").tapped);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityManaPaid { source, ability, .. }
            if *source == snare && *ability == "tap-target-creature"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentTapped { source, card }
            if *source == snare && *card == target
    )));
    game.validate_invariants()
        .expect("Snare activation event sequence is valid");
}

#[test]
fn grifters_blade_equips_at_sorcery_speed_and_keeps_its_persistent_bonus() {
    let mut game = game();
    let blade = game
        .put_on_battlefield(PlayerId(0), "RAV-GRIFTERS-BLADE")
        .expect("Blade enters before the measured game");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("target creature enters");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains enters before the measured game");
    game.begin_game().expect("game starts");
    advance_to_precombat_main(&mut game);
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("equip payment is available");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: blade,
            ability_id: "equip-plus-one-plus-one",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("sorcery-speed Equipment activation is legal");
    resolve_top(&mut game);
    println!("Grifter's Blade trace: {:#?}", game.canonical_event_log());
    assert_eq!(
        game.object(blade).expect("Blade exists").attached_to,
        Some(target)
    );
    let characteristics = game
        .characteristics(target)
        .expect("target characteristics");
    assert_eq!(characteristics.power, Some(4));
    assert_eq!(characteristics.toughness, Some(4));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::EquipmentAttached { equipment, target: attached, previous: None }
            if *equipment == blade && *attached == target
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target: modified, .. }
            if *source == blade && *modified == target
    )));
    game.validate_invariants()
        .expect("Equipment attachment event sequence is valid");
}
