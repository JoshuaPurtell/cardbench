//! Full-fidelity contract for Grozoth.
//!
//! The public scenario retains one frozen immediate-helper compatibility trace;
//! direct Rust coverage below exercises the real stack-backed Transmute path.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, DecisionSelection, Game, GameEvent, Keyword, ManaCost, PlayerId, RulesError,
    Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn grozoth_definition_is_explicit_about_its_full_fidelity_scope() {
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
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "optional-private-multi-card-mana-value-search",
            "stack-backed-private-transmute",
        ]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&grozoth.id),
        "Grozoth's ETB search and stack-backed Transmute are fully represented"
    );
}

#[test]
fn grozoth_rejects_attacking_and_executes_stack_backed_transmute() {
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
        .activate_transmute(PlayerId(0), grozoth_in_hand)
        .expect("Transmute enters the stack");
    assert!(
        !transmute_game.stack.is_empty(),
        "Transmute must create a response window"
    );
    assert_eq!(
        transmute_game.zone_of(grozoth_in_hand),
        Some(Zone::Graveyard)
    );
    assert_eq!(transmute_game.zone_of(matching_value), Some(Zone::Library));
    transmute_game
        .pass_priority(PlayerId(0))
        .expect("controller passes on Transmute");
    transmute_game
        .pass_priority(PlayerId(1))
        .expect("opponent pass opens private Transmute search");
    let decision = transmute_game
        .view_for_player(PlayerId(0))
        .expect("controller view is available")
        .pending_decision
        .expect("Transmute search is a private decision");
    assert!(
        transmute_game
            .view_for_player(PlayerId(1))
            .expect("opponent view is available")
            .pending_decision
            .is_none(),
        "opponent must not see controller-library candidates"
    );
    transmute_game
        .submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Objects(vec![matching_value]),
        )
        .expect("controller selects the matching mana-value card");
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
        .expect("stack-backed Transmute trace preserves engine invariants");

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
fn grozoth_public_scenario_retains_its_frozen_compatibility_receipts_without_attack_declaration() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_grozoth_defender_and_transmute_compatibility")
        .expect("Grozoth public scenario exists");

    println!("Grozoth compatibility trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:c3aa63148bce556d");
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
