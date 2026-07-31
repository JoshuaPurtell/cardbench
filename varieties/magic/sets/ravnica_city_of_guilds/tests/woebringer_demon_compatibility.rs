//! Bounded public contract for Woebringer Demon's shared Flying rule.
//!
//! The printed upkeep sacrifice behavior remains intentionally outside this
//! slice; these tests prove the executable creature chassis and static evasion
//! boundary only.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, CombatBlock, Game, GameEvent, Keyword, ManaCost, PlayerId, RulesError, Step,
    Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn woebringer_demon_definition_is_explicit_about_the_omitted_upkeep_trigger() {
    let demon = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOEBRINGER-DEMON")
        .expect("Woebringer Demon definition exists");

    assert_eq!(demon.name, "Woebringer Demon");
    assert_eq!(
        demon.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Black])
    );
    assert_eq!(demon.colors, BTreeSet::from([Color::Black]));
    assert_eq!(demon.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((demon.power, demon.toughness), (Some(4), Some(4)));
    assert_eq!(demon.keywords, [Keyword::Flying]);
    assert!(demon.effects.is_empty());
    assert_eq!(
        demon.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"]
    );
    assert!(!RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&demon.id));
}

#[test]
fn woebringer_demon_flying_rejects_a_ground_blocker_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let demon = game
        .add_card(PlayerId(0), "RAV-WOEBRINGER-DEMON", Zone::Battlefield)
        .expect("Woebringer Demon begins on battlefield");
    let ground_blocker = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("ground blocker begins on battlefield");
    game.set_entered_turn_for_setup(demon, 0)
        .expect("demon predates the measured turn");
    game.set_entered_turn_for_setup(ground_blocker, 0)
        .expect("blocker predates the measured turn");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[demon])
        .expect("Flying Demon may attack");
    game.pass_priority(PlayerId(0))
        .expect("attacking player passes after declaring attackers");
    game.pass_priority(PlayerId(1))
        .expect("defending player passes into blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);
    let events_before_rejection = game.event_log.clone();

    assert_eq!(
        game.declare_blockers(
            PlayerId(1),
            &[CombatBlock {
                attacker: demon,
                blocker: ground_blocker,
            }],
        ),
        Err(RulesError::IllegalAction(
            "flying attacker can be blocked only by flying or reach"
        ))
    );
    assert_eq!(game.event_log, events_before_rejection);
    assert!(
        game.event_log
            .iter()
            .any(|event| { matches!(event, GameEvent::AttackersDeclared { .. }) })
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| { matches!(event, GameEvent::BlockersDeclared { .. }) })
    );
    game.validate_invariants()
        .expect("rejected Flying block preserves engine invariants");
}

#[test]
fn woebringer_demon_public_scenario_rejects_a_ground_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_woebringer_demon_flying_compatibility")
        .expect("Woebringer Demon public scenario exists");

    println!(
        "Woebringer Demon compatibility trace: {:?}",
        trace.event_log
    );
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
            .expect("active player passes toward combat");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward combat");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    assert_eq!(game.priority, PlayerId(0));
}
