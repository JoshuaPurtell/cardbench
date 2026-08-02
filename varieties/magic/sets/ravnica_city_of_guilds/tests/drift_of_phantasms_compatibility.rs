//! Full-fidelity static and Transmute contract for Drift of Phantasms.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, DecisionKind, DecisionSelection, Game, GameEvent, Keyword, ManaCost, PlayerId,
    RulesError, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_basic_land_type_bindings,
    run_all_scenarios,
};

#[test]
fn drift_definition_is_explicit_about_its_full_static_and_transmute_scope() {
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
            Keyword::Flying,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
        ]
    );
    assert!(drift.effects.is_empty());
    assert_eq!(
        drift.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "flying",
            "transmute",
        ]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&drift.id),
        "stack-backed Transmute and both static keywords are fully represented"
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
fn drift_transmute_uses_the_stack_backed_private_library_decision() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV typed game builds");
    let drift = game
        .add_card(player, "RAV-DRIFT-OF-PHANTASMS", Zone::Hand)
        .expect("Drift begins in hand");
    let matching_value = game
        .add_card(player, "RAV-CHAR", Zone::Library)
        .expect("matching library card");
    let islands = (0..3)
        .map(|_| {
            game.put_on_battlefield(player, "RAV-ISLAND")
                .expect("Transmute mana source")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("advance first priority");
        let second = game.priority;
        game.pass_priority(second).expect("advance second priority");
    }

    for island in islands {
        game.activate_mana_ability(player, island, Color::Blue)
            .expect("Transmute mana");
    }

    game.activate_transmute(player, drift)
        .expect("stack-backed Transmute activates");
    assert_eq!(game.stack.len(), 1, "the ability remains on the stack");
    game.pass_priority(player).expect("controller passes");
    game.pass_priority(opponent)
        .expect("resolution opens the private library decision");
    let decision = game
        .view_for_player(player)
        .expect("controller view")
        .pending_decision
        .expect("private Transmute search choice");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert!(
        game.view_for_player(opponent)
            .expect("opponent view")
            .pending_decision
            .is_none()
    );
    game.submit_decision(
        player,
        decision.id,
        DecisionSelection::Objects(vec![matching_value]),
    )
    .expect("controller selects the matching card");

    println!(
        "drift_stack_transmute_trace={:#?}",
        game.canonical_event_log()
    );
    assert!(game.stack.is_empty());
    assert_eq!(game.zone_of(drift), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(matching_value), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityActivated { source, .. } if *source == drift
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::Transmuted { discarded, found: Some(found), .. }
            if *discarded == drift && *found == matching_value
    )));
    game.validate_invariants()
        .expect("stack-backed Transmute preserves invariants");
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
    assert_eq!(trace.digest, "fnv1a64:d31e3938268687e6");
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
