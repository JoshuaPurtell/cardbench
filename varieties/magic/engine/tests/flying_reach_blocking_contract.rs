//! Expansion-neutral declaration contracts for Flying and Reach.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CombatBlock, Game, Keyword, ManaCost, PlayerId, Step,
};

const FLIER: &str = "TEST-FLYING-ATTACKER";
const FLYING_BLOCKER: &str = "TEST-FLYING-BLOCKER";
const REACH_BLOCKER: &str = "TEST-REACH-BLOCKER";
const GROUND_ATTACKER: &str = "TEST-GROUND-ATTACKER";
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

fn reach_declare_blockers(game: &mut Game, attacker: cardbench_magic_engine::ObjectId) {
    for _ in 0..4 {
        game.pass_priority(game.priority)
            .expect("priority passes reach attacker declaration");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker declaration is legal");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1)).expect("defender passes");
    assert_eq!(game.step, Step::DeclareBlockers);
}

fn game_with_combatants(
    attacker_definition: &'static str,
    attacker_keywords: Vec<Keyword>,
    blocker_definition: &'static str,
    blocker_keywords: Vec<Keyword>,
) -> (
    Game,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let mut game = Game::new(
        vec![
            creature(attacker_definition, attacker_keywords),
            creature(blocker_definition, blocker_keywords),
        ],
        2,
    )
    .expect("test game initializes");
    let attacker = game
        .put_on_battlefield(PlayerId(0), attacker_definition)
        .expect("attacker enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), blocker_definition)
        .expect("blocker enters");
    game.turn = 2;
    reach_declare_blockers(&mut game, attacker);
    (game, attacker, blocker)
}

#[test]
fn flying_and_reach_each_admit_a_flying_blocker_with_a_canonical_receipt() {
    for (blocker_definition, blocker_keywords) in [
        (FLYING_BLOCKER, vec![Keyword::Flying]),
        (REACH_BLOCKER, vec![Keyword::Reach]),
    ] {
        let (mut game, attacker, blocker) = game_with_combatants(
            FLIER,
            vec![Keyword::Flying],
            blocker_definition,
            blocker_keywords,
        );
        game.clear_event_log();

        game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }])
            .expect("a flying attacker accepts Flying or Reach blocker");

        assert_eq!(
            game.canonical_event_log(),
            [format!(
                "BlockersDeclared {{ player: PlayerId(1), assignments: [({attacker:?}, {blocker:?})] }}"
            )],
            "the legal declaration has one durable event receipt"
        );
        assert!(
            game.view_for_player(PlayerId(1))
                .expect("defender view")
                .blockers_declared,
            "legal declaration completes the mandatory combat action"
        );
        game.validate_invariants()
            .expect("accepted Flying/Reach declaration preserves combat provenance");
    }
}

#[test]
fn ordinary_ground_block_remains_legal() {
    let (mut game, attacker, blocker) =
        game_with_combatants(GROUND_ATTACKER, vec![], GROUND_BLOCKER, vec![]);

    game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }])
        .expect("Flying/Reach restriction does not ban ordinary ground blocks");
    game.validate_invariants()
        .expect("ground combat state remains valid");
}
