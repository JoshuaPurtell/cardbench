//! Bounded public contract for Grozoth.
//!
//! This records only the shared Defender and immediate hand-zone Transmute
//! compatibility operations. Its entry trigger and stack-backed activated
//! ability semantics are intentionally not claimed.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, RulesError, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn grozoth_definition_is_explicit_about_its_bounded_compatibility_scope() {
    let grozoth = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GROZOTH")
        .expect("Grozoth definition exists");

    assert_eq!(grozoth.name, "Grozoth");
    assert_eq!(
        grozoth.mana_cost,
        ManaCost::with_colors(6, [Color::Blue, Color::Blue, Color::Blue])
    );
    assert_eq!(grozoth.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(grozoth.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((grozoth.power, grozoth.toughness), (Some(9), Some(9)));
    assert_eq!(
        grozoth.keywords,
        [
            Keyword::Defender,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
        ]
    );
    assert!(grozoth.effects.is_empty());
    assert_eq!(
        grozoth.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "immediate-hand-zone-transmute-compatibility",
        ]
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&grozoth.id),
        "an omitted entry trigger and immediate Transmute prohibit positive fidelity"
    );
}

#[test]
fn grozoth_rejects_attacking_and_executes_the_immediate_transmute_slice() {
    let mut transmute_game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let grozoth_in_hand = transmute_game
        .add_card(PlayerId(0), "RAV-GROZOTH", Zone::Hand)
        .expect("Grozoth begins in hand");
    let matching_value = transmute_game
        .add_card(PlayerId(0), "RAV-GROZOTH", Zone::Library)
        .expect("matching-value card begins in library");
    transmute_game
        .grant_mana(PlayerId(0), Color::Blue, 3)
        .expect("fixture supplies Transmute cost");
    transmute_game.clear_event_log();

    transmute_game
        .transmute(PlayerId(0), grozoth_in_hand, Some(matching_value))
        .expect("bounded immediate Transmute is available");
    assert!(
        transmute_game.stack.is_empty(),
        "this compatibility operation creates no stack object"
    );
    assert_eq!(
        transmute_game.zone_of(grozoth_in_hand),
        Some(Zone::Graveyard)
    );
    assert_eq!(transmute_game.zone_of(matching_value), Some(Zone::Hand));
    assert!(transmute_game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::Transmuted {
                player: PlayerId(0),
                discarded,
                found: Some(found),
            } if *discarded == grozoth_in_hand && *found == matching_value
        )
    }));
    transmute_game
        .validate_invariants()
        .expect("immediate Transmute trace preserves engine invariants");

    let mut defender_game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let defender = defender_game
        .add_card(PlayerId(0), "RAV-GROZOTH", Zone::Battlefield)
        .expect("Grozoth begins on battlefield");
    defender_game
        .set_entered_turn_for_setup(defender, 0)
        .expect("fixture makes Grozoth long-controlled");
    advance_to_declare_attackers(&mut defender_game);
    let events_before_rejection = defender_game.event_log.clone();

    assert_eq!(
        defender_game.declare_attackers(PlayerId(0), &[defender]),
        Err(RulesError::IllegalAction("illegal attacker"))
    );
    assert_eq!(defender_game.event_log, events_before_rejection);
    assert!(
        !defender_game
            .object(defender)
            .expect("Grozoth remains live")
            .tapped
    );
    defender_game
        .validate_invariants()
        .expect("rejected Defender attack preserves engine invariants");
}

#[test]
fn grozoth_public_scenario_records_compatibility_receipts_without_attack_declaration() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_grozoth_defender_and_transmute_compatibility")
        .expect("Grozoth public scenario exists");

    println!("Grozoth compatibility trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:72bfadc28024561c");
    for marker in ["CardRevealed", "LibraryShuffled", "Transmuted", "StepBegan"] {
        assert!(
            trace.event_log.iter().any(|event| event.contains(marker)),
            "Grozoth trace lacks {marker}: {:?}",
            trace.event_log
        );
    }
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("AttackersDeclared")),
        "rejected Defender attack must not write an attack declaration receipt"
    );
}

fn advance_to_declare_attackers(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward combat");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward combat");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    assert_eq!(game.priority, PlayerId(0));
}
