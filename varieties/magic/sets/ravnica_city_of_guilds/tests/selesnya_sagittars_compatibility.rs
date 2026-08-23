//! Public contract for Selesnya Sagittars' static Reach behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, CombatBlock, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn selesnya_sagittars_definition_retains_its_reach_static_rule() {
    let sagittars = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELESNYA-SAGITTARS")
        .expect("Selesnya Sagittars definition exists");

    assert_eq!(sagittars.name, "Selesnya Sagittars");
    assert_eq!(
        sagittars.mana_cost,
        ManaCost::with_colors(3, [Color::Green, Color::White])
    );
    assert_eq!(
        sagittars.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(sagittars.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((sagittars.power, sagittars.toughness), (Some(2), Some(5)));
    assert_eq!(sagittars.keywords, [Keyword::Reach]);
    assert!(sagittars.effects.is_empty());
    assert_eq!(
        sagittars.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "reach",
            "tap-damage-attacking-or-blocking-creature",
        ]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sagittars.id),
        "the represented tap-to-damage activation completes this definition"
    );
}

#[test]
fn selesnya_sagittars_reach_allows_a_flying_blocker() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let attacker = game
        .add_card(PlayerId(0), "RAV-SNAPPING-DRAKE", Zone::Battlefield)
        .expect("Flying attacker begins on the battlefield");
    let blocker = game
        .add_card(PlayerId(1), "RAV-SELESNYA-SAGITTARS", Zone::Battlefield)
        .expect("Reach blocker begins on the battlefield");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker is long-controlled");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("blocker is long-controlled");
    advance_to_declare_blockers(&mut game);

    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("Flying creature attacks");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("defender receives priority");
    game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }])
        .expect("Reach allows blocking the Flying attacker");

    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } if assignments == &[(attacker, blocker)]
        )
    }));
    game.validate_invariants()
        .expect("Reach blocker declaration preserves engine invariants");
}

#[test]
fn selesnya_sagittars_public_scenario_records_reach_qualification() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_selesnya_sagittars_reach_compatibility")
        .expect("Selesnya Sagittars public scenario exists");

    println!(
        "Selesnya Sagittars compatibility trace: {:?}",
        trace.event_log
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("BlockersDeclared"))
    );
}

fn advance_to_declare_blockers(game: &mut Game) {
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
