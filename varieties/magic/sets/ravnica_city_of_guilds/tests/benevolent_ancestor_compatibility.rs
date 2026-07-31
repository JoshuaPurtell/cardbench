//! Bounded public contract for Benevolent Ancestor's shared Defender behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Game, Keyword, ManaCost, PlayerId, RulesError, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn benevolent_ancestor_definition_is_explicit_about_the_omitted_activation() {
    let ancestor = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BENEVOLENT-ANCESTOR")
        .expect("Benevolent Ancestor definition exists");

    assert_eq!(ancestor.name, "Benevolent Ancestor");
    assert_eq!(ancestor.mana_cost, ManaCost::with_colors(2, [Color::White]));
    assert_eq!(ancestor.colors, BTreeSet::from([Color::White]));
    assert_eq!(ancestor.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((ancestor.power, ancestor.toughness), (Some(0), Some(4)));
    assert_eq!(ancestor.keywords, [Keyword::Defender]);
    assert!(ancestor.effects.is_empty());
    assert_eq!(
        ancestor.supported_rules,
        ["colored-cost-casting", "base-characteristics", "defender"]
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&ancestor.id),
        "the omitted damage-prevention activation keeps this definition bounded"
    );
}

#[test]
fn benevolent_ancestor_rejects_attack_declarations_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let ancestor = game
        .add_card(PlayerId(0), "RAV-BENEVOLENT-ANCESTOR", Zone::Battlefield)
        .expect("Benevolent Ancestor begins on battlefield");
    game.set_entered_turn_for_setup(ancestor, 0)
        .expect("fixture makes Benevolent Ancestor long-controlled");
    advance_to_declare_attackers(&mut game);
    let events_before_rejection = game.event_log.clone();

    assert_eq!(
        game.declare_attackers(PlayerId(0), &[ancestor]),
        Err(RulesError::IllegalAction("illegal attacker"))
    );
    assert_eq!(game.event_log, events_before_rejection);
    assert!(!game.object(ancestor).expect("Ancestor remains live").tapped);
    game.validate_invariants()
        .expect("rejected Defender attack preserves engine invariants");
}

#[test]
fn benevolent_ancestor_public_scenario_preserves_no_attack_receipt() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_benevolent_ancestor_defender_compatibility")
        .expect("Benevolent Ancestor public scenario exists");

    println!(
        "Benevolent Ancestor compatibility trace: {:?}",
        trace.event_log
    );
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
