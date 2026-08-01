//! Public contract for Carven Caryatid's Defender and ETB draw slices.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Game, Keyword, ManaCost, PlayerId, RulesError, Step, Zone,
};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
fn carven_caryatid_definition_declares_its_complete_trigger_scope() {
    let caryatid = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CARVEN-CARYATID")
        .expect("Carven Caryatid definition exists");

    assert_eq!(caryatid.name, "Carven Caryatid");
    assert_eq!(
        caryatid.mana_cost,
        ManaCost::with_colors(1, [Color::Green, Color::Green])
    );
    assert_eq!(caryatid.colors, BTreeSet::from([Color::Green]));
    assert_eq!(caryatid.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((caryatid.power, caryatid.toughness), (Some(2), Some(5)));
    assert_eq!(caryatid.keywords, [Keyword::Defender]);
    assert!(caryatid.effects.is_empty());
    assert_eq!(
        caryatid.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "enter-the-battlefield-draw",
        ]
    );
}

#[test]
fn carven_caryatid_rejects_attack_declarations_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let caryatid = game
        .add_card(PlayerId(0), "RAV-CARVEN-CARYATID", Zone::Battlefield)
        .expect("Carven Caryatid begins on battlefield");
    game.set_entered_turn_for_setup(caryatid, 0)
        .expect("fixture makes Carven Caryatid long-controlled");
    advance_to_declare_attackers(&mut game);
    let events_before_rejection = game.event_log.clone();

    assert_eq!(
        game.declare_attackers(PlayerId(0), &[caryatid]),
        Err(RulesError::IllegalAction("illegal attacker"))
    );
    assert_eq!(game.event_log, events_before_rejection);
    assert!(!game.object(caryatid).expect("Caryatid remains live").tapped);
    game.validate_invariants()
        .expect("rejected Defender attack preserves engine invariants");
}

#[test]
fn carven_caryatid_public_scenario_preserves_no_attack_receipt() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_carven_caryatid_defender_compatibility")
        .expect("Carven Caryatid public scenario exists");

    println!("Carven Caryatid compatibility trace: {:?}", trace.event_log);
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
            .any(|event| event.contains("AttackersDeclared"))
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
