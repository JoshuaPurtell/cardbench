//! Red discovery contract for Loxodon Gatekeeper's opposing-permanent entry replacement.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_static_entry_restriction_bindings,
};

fn gatekeeper_game() -> Game {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .expect("RAV entry-restriction bindings register");
    game
}

fn advance_to_main_phase(game: &mut Game, player: PlayerId) {
    for _ in 0..16 {
        if game.active_player == player && game.step.is_main() {
            return;
        }
        let first = game.priority;
        game.pass_priority(first).expect("first player passes");
        let second = game.priority;
        game.pass_priority(second).expect("second player passes");
    }
    panic!("did not advance to the requested main phase");
}

#[test]
fn loxodon_gatekeeper_requires_a_live_opponent_entry_restriction() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LOXODON-GATEKEEPER")
        .expect("Loxodon Gatekeeper definition exists");
    assert_eq!(definition.name, "Loxodon Gatekeeper");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::White, Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition
        .supported_rules
        .contains(&"static-opponents-artifacts-creatures-lands-enter-tapped"));
}

#[test]
fn loxodon_gatekeeper_taps_an_opponents_creature_on_ordinary_stack_entry() {
    let mut game = gatekeeper_game();
    let gatekeeper = game
        .put_on_battlefield(PlayerId(0), "RAV-LOXODON-GATEKEEPER")
        .expect("Gatekeeper enters during setup");
    let watchwolf = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Hand)
        .expect("opponent has Watchwolf");
    let forest = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("opponent Forest enters during setup");
    let plains = game
        .put_on_battlefield(PlayerId(1), "RAV-PLAINS")
        .expect("opponent Plains enters during setup");
    game.begin_game().expect("game starts");
    advance_to_main_phase(&mut game, PlayerId(1));
    game.activate_mana_ability(PlayerId(1), forest, Color::Green)
        .expect("Forest provides green");
    game.activate_mana_ability(PlayerId(1), plains, Color::White)
        .expect("Plains provides white");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent casts Watchwolf");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes Watchwolf");
    game.pass_priority(PlayerId(0))
        .expect("Gatekeeper controller allows Watchwolf to resolve");

    assert_eq!(game.zone_of(watchwolf), Some(Zone::Battlefield));
    assert!(game.object(watchwolf).expect("Watchwolf lives").tapped);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentEnteredTapped { permanent, source, .. }
            if *permanent == watchwolf && *source == gatekeeper
    )));
    eprintln!("Loxodon Gatekeeper trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("entry restriction trace preserves invariants");
}

