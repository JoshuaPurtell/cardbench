//! Bounded public contract for Undercity Shade's black-only evasion.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, CombatBlock, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn undercity_shade_definition_is_explicit_about_evasion_and_omitted_activation() {
    let shade = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-UNDERCITY-SHADE")
        .expect("Undercity Shade definition exists");
    assert_eq!(shade.name, "Undercity Shade");
    assert_eq!(shade.mana_cost, ManaCost::with_colors(4, [Color::Black]));
    assert_eq!(shade.colors, BTreeSet::from([Color::Black]));
    assert_eq!(shade.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((shade.power, shade.toughness), (Some(1), Some(1)));
    assert_eq!(shade.keywords, [Keyword::BlackEvasion]);
    assert!(shade.effects.is_empty());
    assert_eq!(
        shade.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "black-only-evasion"
        ]
    );
    assert!(!RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&shade.id));
}

#[test]
fn undercity_shade_public_scenario_rejects_a_nonblack_blocker() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_undercity_shade_black_evasion_compatibility")
        .expect("Undercity Shade public scenario exists");
    println!("Undercity Shade compatibility trace: {:?}", trace.event_log);
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

#[test]
fn undercity_shade_accepts_a_black_blocker() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let shade = game
        .add_card(PlayerId(0), "RAV-UNDERCITY-SHADE", Zone::Battlefield)
        .expect("Undercity Shade begins on battlefield");
    let black_blocker = game
        .add_card(PlayerId(1), "RAV-SEWERDREG", Zone::Battlefield)
        .expect("black blocker begins on battlefield");
    game.set_entered_turn_for_setup(shade, 0)
        .expect("shade predates the measured turn");
    game.set_entered_turn_for_setup(black_blocker, 0)
        .expect("black blocker predates the measured turn");
    advance_to_blockers(&mut game, shade);
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: shade,
            blocker: black_blocker,
        }],
    )
    .expect("black creatures may block a black-only evasion attacker");
    assert!(
        game.event_log
            .iter()
            .any(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
    );
    game.validate_invariants()
        .expect("accepted black block preserves engine invariants");
}

fn advance_to_blockers(game: &mut Game, shade: cardbench_magic_engine::ObjectId) {
    game.begin_game().expect("fixture starts game");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward combat");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward combat");
    }
    game.declare_attackers(PlayerId(0), &[shade])
        .expect("shade may attack");
    game.pass_priority(PlayerId(0))
        .expect("attacker passes after declaration");
    game.pass_priority(PlayerId(1))
        .expect("defender passes into blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);
}
