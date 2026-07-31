//! Red regression for flying's declaration-time blocking restriction.
//!
//! The keyword enum exists so this test can compile, but the current blocker
//! declaration accepts an ordinary ground creature against a flier.  This is a
//! real combat-rules defect, not a request to approximate unrelated evasion.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CombatBlock, Game, Keyword, ManaCost, PlayerId, RulesError, Step,
};

const FLIER: &str = "TEST-FLYING-ATTACKER";
const GROUND_BLOCKER: &str = "TEST-GROUND-BLOCKER";

fn creature(id: &'static str, keywords: Vec<Keyword>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics", "test-combat"],
        power: Some(2),
        toughness: Some(2),
        keywords,
        effects: vec![],
    }
}

fn reach_declare_attackers(game: &mut Game) {
    for _ in 0..4 {
        game.pass_priority(game.priority)
            .expect("priority passes reach attacker declaration");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

#[test]
fn ground_creature_cannot_block_a_flying_attacker_and_rejection_is_atomic() {
    let attacker_controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new(
        vec![
            creature(FLIER, vec![Keyword::Flying]),
            creature(GROUND_BLOCKER, vec![]),
        ],
        2,
    )
    .expect("test game initializes");
    let flier = game
        .put_on_battlefield(attacker_controller, FLIER)
        .expect("flier enters");
    let ground = game
        .put_on_battlefield(defender, GROUND_BLOCKER)
        .expect("ground creature enters");
    // This authored pregame fixture represents a later turn, so neither
    // creature is summoning sick when combat begins.
    game.turn = 2;
    reach_declare_attackers(&mut game);
    game.declare_attackers(attacker_controller, &[flier])
        .expect("flying attacker is legal");
    game.pass_priority(attacker_controller)
        .expect("attacker passes");
    game.pass_priority(defender).expect("defender passes");
    assert_eq!(game.step, Step::DeclareBlockers);
    let events_before = game.canonical_event_log().clone();

    let result = game.declare_blockers(
        defender,
        &[CombatBlock {
            attacker: flier,
            blocker: ground,
        }],
    );

    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction(
                "flying attacker can be blocked only by flying or reach"
            ))
        ),
        "a ground blocker was accepted against a flier; result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.canonical_event_log(),
        events_before,
        "an illegal block declaration must not write a BlockersDeclared receipt"
    );
    assert!(
        !game
            .view_for_player(defender)
            .expect("defender view")
            .blockers_declared,
        "rejected declaration must not advance the combat state machine"
    );
    game.validate_invariants()
        .expect("failed declaration leaves a valid pre-block state");
}
