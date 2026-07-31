//! Bounded public contract for Divebomber Griffin's shared Flying behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, CombatBlock, Game, Keyword, ManaCost, PlayerId, RulesError, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn divebomber_griffin_definition_is_explicit_about_the_omitted_activation() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIVEBOMBER-GRIFFIN")
        .expect("Divebomber Griffin definition exists");

    assert_eq!(griffin.name, "Divebomber Griffin");
    assert_eq!(
        griffin.mana_cost,
        ManaCost::with_colors(3, [Color::White, Color::White])
    );
    assert_eq!(griffin.colors, BTreeSet::from([Color::White]));
    assert_eq!(griffin.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((griffin.power, griffin.toughness), (Some(3), Some(2)));
    assert_eq!(griffin.keywords, [Keyword::Flying]);
    assert!(griffin.effects.is_empty());
    assert_eq!(
        griffin.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
    assert!(!RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&griffin.id));
}

#[test]
fn divebomber_griffin_flying_rejects_a_ground_blocker_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let griffin = game
        .add_card(PlayerId(0), "RAV-DIVEBOMBER-GRIFFIN", Zone::Battlefield)
        .expect("Divebomber Griffin begins on battlefield");
    let blocker = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("ground blocker begins on battlefield");
    game.set_entered_turn_for_setup(griffin, 0)
        .expect("fixture makes Griffin long-controlled");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("fixture makes blocker long-controlled");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[griffin])
        .expect("Flying Griffin can attack");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("defender receives priority");
    let events_before_rejection = game.event_log.clone();

    assert_eq!(
        game.declare_blockers(
            PlayerId(1),
            &[CombatBlock {
                attacker: griffin,
                blocker,
            }],
        ),
        Err(RulesError::IllegalAction(
            "flying attacker can be blocked only by flying or reach",
        ))
    );
    assert_eq!(game.event_log, events_before_rejection);
    game.validate_invariants()
        .expect("rejected ground block preserves engine invariants");
}

#[test]
fn divebomber_griffin_public_scenario_preserves_flying_receipts() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_divebomber_griffin_flying_compatibility")
        .expect("Divebomber Griffin public scenario exists");
    assert_eq!(trace.digest, "fnv1a64:1dcdb2289fdaa917");
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

fn advance_to_declare_attackers(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes");
        game.pass_priority(PlayerId(1)).expect("opponent passes");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    assert_eq!(game.priority, PlayerId(0));
}
