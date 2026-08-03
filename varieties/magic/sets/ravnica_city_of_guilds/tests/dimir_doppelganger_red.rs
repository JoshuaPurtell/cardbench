//! Red contract for Dimir Doppelganger's graveyard-copy activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

const COPY_ABILITY: &str = "exile-graveyard-creature-card-copy-source";

fn doppelganger_game() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture constructs")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("controller passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("opponent passes and resolves");
}

fn add_doppelganger_mana_sources(
    game: &mut Game,
    controller: PlayerId,
    copies: usize,
) -> Vec<cardbench_magic_engine::ObjectId> {
    let mut lands = Vec::new();
    for _ in 0..copies {
        // The payment algorithm intentionally selects generic mana before
        // colored symbols, so provide a second copy of each colored source
        // rather than smuggling a live-game mana-pool mutation into setup.
        for definition in ["RAV-ISLAND", "RAV-ISLAND", "RAV-SWAMP", "RAV-SWAMP"] {
            lands.push(
                game.put_on_battlefield(controller, definition)
                    .expect("colored mana source setup"),
            );
        }
    }
    lands
}

fn activate_doppelganger_mana(
    game: &mut Game,
    controller: PlayerId,
    lands: &[cardbench_magic_engine::ObjectId],
) {
    for (index, land) in lands.iter().copied().enumerate() {
        let color = match index % 4 {
            0 | 1 => Color::Blue,
            2 | 3 => Color::Black,
            _ => unreachable!("modulo is bounded"),
        };
        game.activate_mana_ability(controller, land, color)
            .expect("typed mana source activates");
    }
}

#[test]
fn dimir_doppelganger_has_a_full_fidelity_graveyard_copy_definition() {
    assert_eq!(
        executable_definition_id_for_collector(202),
        Ok("RAV-DIMIR-DOPPELGANGER")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-DOPPELGANGER")
        .expect("Dimir Doppelganger definition exists");
    assert_eq!(definition.name, "Dimir Doppelganger");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue, Color::Black])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Blue, Color::Black])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(definition.power, Some(0));
    assert_eq!(definition.toughness, Some(2));
    assert!(
        definition
            .supported_rules
            .contains(&"exile-target-creature-card-from-any-graveyard-copy-source")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"copied-form-retains-graveyard-copy-activation")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}

#[test]
fn doppelganger_copies_any_graveyard_creature_then_retains_its_physical_ability() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = doppelganger_game();
    let doppelganger = game
        .put_on_battlefield(controller, "RAV-DIMIR-DOPPELGANGER")
        .expect("Doppelganger setup");
    let watchwolf = game
        .add_card(opponent, "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("opponent graveyard target setup");
    let recruit = game
        .add_card(opponent, "RAV-BOROS-RECRUIT", Zone::Graveyard)
        .expect("second opponent graveyard target setup");
    let lands = add_doppelganger_mana_sources(&mut game, controller, 2);

    game.begin_game().expect("fixture begins game");
    activate_doppelganger_mana(&mut game, controller, &lands);
    game.clear_event_log();

    game.activate_ability(
        controller,
        AbilityActivation {
            source: doppelganger,
            ability_id: COPY_ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(watchwolf)],
        },
    )
    .expect("first graveyard-copy activation enters the stack");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(watchwolf), Some(Zone::Exile));
    assert_eq!(
        game.characteristics(doppelganger)
            .expect("copied source remains live")
            .power,
        Some(3)
    );
    assert!(
        game.object(doppelganger)
            .expect("copied source object")
            .retained_activated_abilities
            .iter()
            .any(|retained| retained.definition == "RAV-DIMIR-DOPPELGANGER"
                && retained.ability == COPY_ABILITY)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentCopied { source, target, .. }
            if *source == watchwolf && *target == doppelganger
    )));

    game.activate_ability(
        controller,
        AbilityActivation {
            source: doppelganger,
            ability_id: COPY_ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(recruit)],
        },
    )
    .expect("copied form retains Doppelganger's activated ability");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(recruit), Some(Zone::Exile));
    let characteristics = game
        .characteristics(doppelganger)
        .expect("second copied source remains live");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(1), Some(1))
    );
    assert!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::AbilityActivated { source, definition, ability, .. }
                    if *source == doppelganger
                        && *definition == "RAV-DIMIR-DOPPELGANGER"
                        && *ability == COPY_ABILITY
            ))
            .count()
            == 2
    );
    game.validate_invariants()
        .expect("copied activation retains valid provenance");
    eprintln!(
        "Dimir Doppelganger repeated-copy trace={:?}",
        game.canonical_event_log()
    );
}

#[test]
fn doppelganger_copied_zero_zero_receives_terminal_ability_receipt_before_sbas() {
    let controller = PlayerId(0);
    let mut game = doppelganger_game();
    let doppelganger = game
        .put_on_battlefield(controller, "RAV-DIMIR-DOPPELGANGER")
        .expect("Doppelganger setup");
    let grave_troll = game
        .add_card(PlayerId(1), "RAV-GOLGARI-GRAVE-TROLL", Zone::Graveyard)
        .expect("zero-zero graveyard target setup");
    let lands = add_doppelganger_mana_sources(&mut game, controller, 1);

    game.begin_game().expect("fixture begins game");
    activate_doppelganger_mana(&mut game, controller, &lands);
    game.clear_event_log();
    game.activate_ability(
        controller,
        AbilityActivation {
            source: doppelganger,
            ability_id: COPY_ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(grave_troll)],
        },
    )
    .expect("zero-zero copy activation enters the stack");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(grave_troll), Some(Zone::Exile));
    assert_eq!(game.zone_of(doppelganger), Some(Zone::Graveyard));
    let copy_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::PermanentCopied { source, target, .. }
                    if *source == grave_troll && *target == doppelganger
            )
        })
        .expect("copy receipt exists");
    let resolved_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == doppelganger && *ability == COPY_ABILITY
            )
        })
        .expect("terminal ability receipt exists");
    let sba_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::StateBasedAction { card, reason }
                    if *card == doppelganger && *reason == "creature has toughness zero or less"
            )
        })
        .expect("zero-zero source is processed by an SBA");
    assert!(copy_index < resolved_index && resolved_index < sba_index);
    assert!(
        game.object(doppelganger)
            .expect("departed card object")
            .retained_activated_abilities
            .is_empty()
    );
    game.validate_invariants()
        .expect("post-SBA departed copy remains invariant-valid");
    eprintln!(
        "Dimir Doppelganger zero-zero trace={:?}",
        game.canonical_event_log()
    );
}
