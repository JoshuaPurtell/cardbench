//! Compatibility contract for Blood Funnel's cost and cast-trigger substrate.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_funnel_rules() -> Game {
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
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .expect("Blood Funnel reduction registers");
    game
}

#[test]
fn noncreature_cast_is_reduced_and_trigger_sacrifices_before_spell_resolution() {
    let mut game = game_with_funnel_rules();
    let funnel = game
        .add_card(PlayerId(0), "RAV-BLOOD-FUNNEL", Zone::Battlefield)
        .expect("Blood Funnel setup");
    let fodder = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("creature setup");
    let spell = game
        .add_card(PlayerId(0), "RAV-FISTS-OF-IRONWOOD", Zone::Hand)
        .expect("noncreature spell setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("reduced spell mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Blood Funnel reduces the spell's generic cost");
    assert_eq!(game.stack.len(), 2, "the trigger is above its spell");
    game.pass_priority(PlayerId(0))
        .expect("caster passes trigger");
    game.pass_priority(PlayerId(1))
        .expect("trigger resolves first");
    assert_eq!(game.zone_of(fodder), Some(Zone::Graveyard));
    assert_eq!(
        game.stack.len(),
        1,
        "the sacrificed branch keeps spell live"
    );
    game.pass_priority(PlayerId(0))
        .expect("caster passes spell");
    game.pass_priority(PlayerId(1)).expect("spell resolves");

    println!(
        "Blood Funnel sacrifice trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedByEffect { source, player, permanent }
            if *source == funnel && *player == PlayerId(0) && *permanent == fodder
    )));
    game.validate_invariants()
        .expect("sacrifice branch preserves invariants");
}

#[test]
fn noncreature_cast_without_a_creature_is_countered_by_its_own_trigger() {
    let mut game = game_with_funnel_rules();
    let funnel = game
        .add_card(PlayerId(0), "RAV-BLOOD-FUNNEL", Zone::Battlefield)
        .expect("Blood Funnel setup");
    let spell = game
        .add_card(PlayerId(0), "RAV-FISTS-OF-IRONWOOD", Zone::Hand)
        .expect("noncreature spell setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("reduced spell mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell and trigger enter stack");
    game.pass_priority(PlayerId(0))
        .expect("caster passes trigger");
    game.pass_priority(PlayerId(1))
        .expect("trigger counters spell");

    println!(
        "Blood Funnel counter trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 0);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCountered { card, source } if *card == spell && *source == funnel
    )));
    game.validate_invariants()
        .expect("counter branch preserves invariants");
}

#[test]
fn creature_cast_is_not_reduced_or_triggered() {
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-BLOOD-FUNNEL"),
        "the deterministic sacrifice selection remains an explicit policy gap"
    );
    let mut game = game_with_funnel_rules();
    let funnel = game
        .add_card(PlayerId(0), "RAV-BLOOD-FUNNEL", Zone::Battlefield)
        .expect("Blood Funnel setup");
    let creature = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("creature setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("green cost");
    game.grant_mana(PlayerId(0), Color::White, 1)
        .expect("white cost");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: creature,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("creature pays its unchanged cost");
    assert_eq!(game.stack.len(), 1);
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, .. } if *source == funnel
    )));
    game.pass_priority(PlayerId(0))
        .expect("caster passes creature");
    game.pass_priority(PlayerId(1)).expect("creature resolves");
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("creature branch preserves invariants");
}
