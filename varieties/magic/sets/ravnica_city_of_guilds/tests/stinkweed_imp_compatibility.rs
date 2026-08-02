//! Static-keyword regression contract for ability-complete Stinkweed Imp.
//!
//! Its separate full-fidelity contract covers the source-bound combat-damage
//! trigger. These tests retain direct coverage for the static Flying and Dredge
//! characteristics that share the same card definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, RulesError, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn stinkweed_imp_definition_retains_its_static_characteristics() {
    let imp = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STINKWEED-IMP")
        .expect("Stinkweed Imp definition exists");

    assert_eq!(imp.name, "Stinkweed Imp");
    assert_eq!(imp.mana_cost, ManaCost::with_colors(2, [Color::Black]));
    assert_eq!(imp.colors, BTreeSet::from([Color::Black]));
    assert_eq!(imp.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((imp.power, imp.toughness), (Some(1), Some(2)));
    assert_eq!(imp.keywords, [Keyword::Flying, Keyword::Dredge(5)]);
    assert!(imp.effects.is_empty());
    assert_eq!(
        imp.supported_rules,
        [
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "flying",
            "combat-damage-destroy-recipient",
        ]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&imp.id),
        "the explicit combat-damage trigger makes this definition ability-complete"
    );
}

#[test]
fn stinkweed_imp_flying_rejects_a_ground_blocker_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let imp = game
        .add_card(PlayerId(0), "RAV-STINKWEED-IMP", Zone::Battlefield)
        .expect("Stinkweed Imp begins on battlefield");
    let ground_blocker = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("ground blocker begins on battlefield");
    game.set_entered_turn_for_setup(imp, 0)
        .expect("imp predates the measured turn");
    game.set_entered_turn_for_setup(ground_blocker, 0)
        .expect("blocker predates the measured turn");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[imp])
        .expect("Flying Imp may attack");
    game.pass_priority(PlayerId(0))
        .expect("attacking player passes after declaring attackers");
    game.pass_priority(PlayerId(1))
        .expect("defending player passes into blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);
    let events_before_rejection = game.event_log.clone();

    assert_eq!(
        game.declare_blockers(
            PlayerId(1),
            &[cardbench_magic_engine::CombatBlock {
                attacker: imp,
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
            .any(|event| matches!(event, GameEvent::AttackersDeclared { .. }))
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
fn stinkweed_imp_public_scenario_rejects_a_ground_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_stinkweed_imp_flying_compatibility")
        .expect("Stinkweed Imp public scenario exists");

    println!("Stinkweed Imp compatibility trace: {:?}", trace.event_log);
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
            .any(|event| event.contains("BlockersDeclared")),
        "an illegal ground block must not write a blocker declaration receipt"
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
