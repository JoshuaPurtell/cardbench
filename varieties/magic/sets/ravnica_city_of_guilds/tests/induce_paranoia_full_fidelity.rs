//! Event-log contracts for Induce Paranoia's counter and captured mill path.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, ManaPaymentSelection, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

#[test]
#[allow(clippy::too_many_lines)] // The counter/mill response trace is the contract.
fn induce_paranoia_counters_a_physical_spell_then_mills_that_spells_controller() {
    check_counter_and_mill(true);
}

#[test]
fn induce_paranoia_without_black_counters_without_milling() {
    check_counter_and_mill(false);
}

fn check_counter_and_mill(spend_black: bool) {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = game_with_rav_bindings();
    let watchwolf = game
        .add_card(caster, "RAV-WATCHWOLF", Zone::Hand)
        .expect("target creature spell");
    let induce = game
        .add_card(responder, "RAV-INDUCE-PARANOIA", Zone::Hand)
        .expect("Induce Paranoia setup");
    let library_bottom = game
        .add_card(caster, "RAV-PLAINS", Zone::Library)
        .expect("bottom mill card");
    let library_top = game
        .add_card(caster, "RAV-FOREST", Zone::Library)
        .expect("top mill card");
    let plains = game
        .put_on_battlefield(caster, "RAV-PLAINS")
        .expect("Watchwolf white source");
    let forest = game
        .put_on_battlefield(caster, "RAV-FOREST")
        .expect("Watchwolf green source");
    let islands = (0..3)
        .map(|_| {
            game.put_on_battlefield(responder, "RAV-ISLAND")
                .expect("Induce Paranoia blue source")
        })
        .collect::<Vec<_>>();
    let fourth = game.put_on_battlefield(responder,
        if spend_black { "RAV-SWAMP" } else { "RAV-ISLAND" }).unwrap();

    game.begin_game().expect("game begins");
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("advance first priority");
        let second = game.priority;
        game.pass_priority(second).expect("advance second priority");
    }
    game.activate_mana_ability(caster, plains, Color::White)
        .expect("white mana");
    game.activate_mana_ability(caster, forest, Color::Green)
        .expect("green mana");
    game.cast_spell(
        caster,
        CastRequest {
            card: watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Watchwolf casts");
    game.pass_priority(caster)
        .expect("caster opens response window");
    for island in islands {
        game.activate_mana_ability(responder, island, Color::Blue)
            .expect("blue mana");
    }
    let fourth_color = if spend_black { Color::Black } else { Color::Blue };
    game.activate_mana_ability(responder, fourth, fourth_color).unwrap();
    game.cast_spell_with_mana_spend(
        responder,
        CastRequest {
            card: induce,
            targets: vec![Target::Spell(watchwolf)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![Color::Blue, fourth_color],
            hybrid: vec![],
        },
    )
    .expect("Induce Paranoia casts with its explicit mana-spend receipt");
    game.pass_priority(responder).expect("responder passes");
    game.pass_priority(caster)
        .expect("Induce Paranoia resolves");

    println!(
        "induce_paranoia_event_log={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(watchwolf), Some(Zone::Graveyard));
    let library_destination = if spend_black { Zone::Graveyard } else { Zone::Library };
    assert_eq!(game.zone_of(library_top), Some(library_destination));
    assert_eq!(game.zone_of(library_bottom), Some(library_destination));
    assert_eq!(game.zone_of(induce), Some(Zone::Graveyard));
    game.validate_invariants().unwrap();
    if !spend_black { return; }

    let counter_index = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::SpellCountered { card, source } if *card == watchwolf && *source == induce
        ))
        .expect("counter receipt");
    let watchwolf_terminal_move = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == watchwolf
            )
        })
        .expect("target terminal move");
    let mill_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == library_top
            )
        })
        .expect("first mill move");
    let resolved_index = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == induce))
        .expect("source resolution receipt");
    assert!(
        counter_index < watchwolf_terminal_move
            && watchwolf_terminal_move < mill_index
            && mill_index < resolved_index,
        "counter terminal move and captured-controller mill must precede source resolution"
    );
    game.validate_invariants()
        .expect("Induce Paranoia trace preserves stack, target, zone, and payment invariants");
}

#[test]
fn induce_paranoia_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-INDUCE-PARANOIA"));
}
