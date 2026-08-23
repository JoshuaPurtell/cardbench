//! Public contract for Torpid Moloch's Defender behavior and full-fidelity marker.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Game, Keyword, ManaCost, PlayerId, RulesError, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn torpid_moloch_definition_is_explicit_about_its_activated_ability() {
    let moloch = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TORPID-MOLOCH")
        .expect("Torpid Moloch definition exists");

    assert_eq!(moloch.name, "Torpid Moloch");
    assert_eq!(moloch.mana_cost, ManaCost::with_colors(0, [Color::Red]));
    assert_eq!(moloch.colors, BTreeSet::from([Color::Red]));
    assert_eq!(moloch.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((moloch.power, moloch.toughness), (Some(3), Some(2)));
    assert_eq!(moloch.keywords, [Keyword::Defender]);
    assert!(moloch.effects.is_empty());
    assert!(
        moloch
            .supported_rules
            .contains(&"sacrifice-three-lands-remove-defender")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&moloch.id));
}

#[test]
fn torpid_moloch_rejects_attack_declarations_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let moloch = game
        .add_card(PlayerId(0), "RAV-TORPID-MOLOCH", Zone::Battlefield)
        .expect("Torpid Moloch begins on battlefield");
    game.set_entered_turn_for_setup(moloch, 0)
        .expect("fixture makes Torpid Moloch long-controlled");
    advance_to_declare_attackers(&mut game);
    let events_before_rejection = game.event_log.clone();

    assert_eq!(
        game.declare_attackers(PlayerId(0), &[moloch]),
        Err(RulesError::IllegalAction("illegal attacker"))
    );
    assert_eq!(game.event_log, events_before_rejection);
    assert!(!game.object(moloch).expect("Moloch remains live").tapped);
    game.validate_invariants()
        .expect("rejected Defender attack preserves engine invariants");
}

#[test]
fn torpid_moloch_public_scenario_preserves_no_attack_receipt() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_torpid_moloch_defender_compatibility")
        .expect("Torpid Moloch public scenario exists");

    println!("Torpid Moloch compatibility trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:582e991a894fa999");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("StepBegan"))
    );
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("AttackersDeclared")),
        "rejected Defender attack must not emit an attack receipt"
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
