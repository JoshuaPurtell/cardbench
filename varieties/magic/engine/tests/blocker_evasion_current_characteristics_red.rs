//! RED regression: blocker legality must use the attacker's characteristics
//! at blocker declaration, not its characteristics when it was declared.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CombatBlock, ContinuousChange, Duration, Game, Keyword, ManaCost,
    PlayerId, RulesError, Step,
};

const ATTACKER: &str = "TST-EVASION-CHANGES-ATTACKER";
const GROUND_BLOCKER: &str = "TST-EVASION-CHANGES-GROUND";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics", "continuous-keyword", "test-combat"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
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
fn gaining_flying_after_attackers_are_declared_restricts_ground_blockers() {
    let attacker_controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new(vec![creature(ATTACKER), creature(GROUND_BLOCKER)], 2)
        .expect("two-player fixture initializes");
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker enters");
    let ground = game
        .put_on_battlefield(defender, GROUND_BLOCKER)
        .expect("ground blocker enters");
    game.turn = 2;
    reach_declare_attackers(&mut game);
    game.declare_attackers(attacker_controller, &[attacker])
        .expect("ground attacker is legally declared");

    game.add_continuous_effect(
        attacker,
        attacker,
        ContinuousChange::AddKeyword(Keyword::Flying),
        Duration::EndOfTurn(game.turn),
    )
    .expect("attacker gains flying before blockers are declared");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes");
    game.pass_priority(defender)
        .expect("defender reaches blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);

    let result = game.declare_blockers(
        defender,
        &[CombatBlock {
            attacker,
            blocker: ground,
        }],
    );
    println!(
        "post-declaration flying red result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction(
                "flying attacker can be blocked only by flying or reach"
            ))
        ),
        "a newly flying attacker must reject a ground blocker at declaration time"
    );
    game.validate_invariants()
        .expect("rejected current-characteristics block keeps combat valid");
}
