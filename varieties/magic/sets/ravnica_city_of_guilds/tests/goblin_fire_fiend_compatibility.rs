//! Public contract for Goblin Fire Fiend's complete represented behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Game, Keyword, ManaCost, PlayerId, Step, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn goblin_fire_fiend_definition_is_explicit_about_omitted_behaviors() {
    let fiend = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-FIRE-FIEND")
        .expect("Goblin Fire Fiend definition exists");

    assert_eq!(fiend.name, "Goblin Fire Fiend");
    assert_eq!(fiend.mana_cost, ManaCost::with_colors(3, [Color::Red]));
    assert_eq!(fiend.colors, BTreeSet::from([Color::Red]));
    assert_eq!(fiend.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((fiend.power, fiend.toughness), (Some(1), Some(1)));
    assert_eq!(
        fiend.keywords,
        [Keyword::Haste, Keyword::MustBeBlockedIfAble]
    );
    assert!(fiend.effects.is_empty());
    assert_eq!(
        fiend.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "haste",
            "must-block-if-able",
            "activated-plus-one-power",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&fiend.id));
}

#[test]
fn goblin_fire_fiend_public_trace_exercises_same_turn_haste_attack() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_goblin_fire_fiend_haste_compatibility")
        .expect("Goblin Fire Fiend public trace");
    println!("Goblin Fire Fiend trace: {:?}", trace.event_log);
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("SpellCast"))
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("AttackersDeclared"))
    );
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("BlockersDeclared"))
    );
}

#[test]
fn goblin_fire_fiend_haste_attack_is_atomic_in_the_shared_engine() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let fiend = game
        .put_on_battlefield(PlayerId(0), "RAV-GOBLIN-FIRE-FIEND")
        .expect("Goblin Fire Fiend begins on battlefield");
    game.set_entered_turn_for_setup(fiend, 0)
        .expect("fixture makes the creature long-controlled");
    game.begin_game().expect("fixture starts");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0)).expect("active pass");
        game.pass_priority(PlayerId(1)).expect("response pass");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[fiend])
        .expect("Haste creature can attack on its entry turn");
    assert_eq!(game.zone_of(fiend), Some(Zone::Battlefield));
    assert!(game.object(fiend).expect("fiend remains live").tapped);
    game.validate_invariants()
        .expect("same-turn Haste attack preserves invariants");
}
