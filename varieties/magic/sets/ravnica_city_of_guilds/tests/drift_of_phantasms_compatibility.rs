//! Bounded public contract for Drift of Phantasms.
//!
//! This test deliberately distinguishes the represented Defender and immediate
//! hand-zone Transmute operations from stack-backed activated-ability fidelity.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, RulesError, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn drift_definition_is_explicit_about_its_bounded_defender_and_transmute_scope() {
    let drift = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DRIFT-OF-PHANTASMS")
        .expect("Drift of Phantasms definition exists");

    assert_eq!(drift.name, "Drift of Phantasms");
    assert_eq!(drift.mana_cost, ManaCost::with_colors(2, [Color::Blue]));
    assert_eq!(drift.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(drift.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((drift.power, drift.toughness), (Some(0), Some(5)));
    assert_eq!(
        drift.keywords,
        [
            Keyword::Defender,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
        ]
    );
    assert!(drift.effects.is_empty());
    assert_eq!(
        drift.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "immediate-hand-zone-transmute-compatibility",
        ]
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&drift.id),
        "immediate Transmute must not be mistaken for its stack-backed printed behavior"
    );
}

#[test]
fn drift_directly_rejects_attacking_and_executes_only_the_existing_immediate_transmute() {
    let mut transmute_game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let drift_in_hand = transmute_game
        .add_card(PlayerId(0), "RAV-DRIFT-OF-PHANTASMS", Zone::Hand)
        .expect("Drift begins in hand");
    let matching_value = transmute_game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Library)
        .expect("same-value card begins in library");
    transmute_game
        .grant_mana(PlayerId(0), Color::Blue, 3)
        .expect("fixture supplies the selected activation cost");
    transmute_game.clear_event_log();

    transmute_game
        .transmute(PlayerId(0), drift_in_hand, Some(matching_value))
        .expect("the bounded immediate Transmute operation is available");
    assert!(
        transmute_game.stack.is_empty(),
        "the compatibility operation creates no stack object"
    );
    assert_eq!(transmute_game.zone_of(drift_in_hand), Some(Zone::Graveyard));
    assert_eq!(transmute_game.zone_of(matching_value), Some(Zone::Hand));
    assert!(transmute_game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::Transmuted {
                player: PlayerId(0),
                discarded,
                found: Some(found),
            } if *discarded == drift_in_hand && *found == matching_value
        )
    }));
    transmute_game
        .validate_invariants()
        .expect("immediate Transmute trace preserves engine invariants");

    let mut defender_game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let defender = defender_game
        .add_card(PlayerId(0), "RAV-DRIFT-OF-PHANTASMS", Zone::Battlefield)
        .expect("Drift begins on the battlefield");
    defender_game
        .set_entered_turn_for_setup(defender, 0)
        .expect("fixture makes Drift long-controlled");
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
            .expect("Drift remains live")
            .tapped
    );
    defender_game
        .validate_invariants()
        .expect("rejected Defender attack preserves engine invariants");
}

#[test]
fn drift_public_scenario_records_the_compatibility_receipt_and_no_attack_declaration() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| {
            scenario.id == "rav_drift_of_phantasms_defender_and_transmute_compatibility"
        })
        .expect("Drift public scenario exists");

    println!("Drift compatibility trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:74b1293dcdd0928b");
    for marker in ["CardRevealed", "LibraryShuffled", "Transmuted", "StepBegan"] {
        assert!(
            trace.event_log.iter().any(|event| event.contains(marker)),
            "Drift trace lacks {marker}: {:?}",
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
