//! Red regression: a landwalk attacker cannot create a false must-block
//! requirement when the defender controls the matching basic-land type.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardType, ContinuousChange, Duration,
    Game, Keyword, ManaCost, PlayerId, RulesError, Step,
};

const ATTACKER: &str = "TST-MUST-BLOCK-LANDWALK-ATTACKER";
const BLOCKER: &str = "TST-MUST-BLOCK-LANDWALK-BLOCKER";
const FOREST: &str = "TST-MUST-BLOCK-LANDWALK-FOREST";

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
        supported_rules: &["must-block-landwalk-red"],
        power: Some(2),
        toughness: Some(2),
        keywords,
        effects: vec![],
    }
}

fn forest() -> CardDefinition {
    CardDefinition {
        id: FOREST,
        name: FOREST,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([cardbench_magic_engine::Color::Green]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: true,
        supported_rules: &["typed-basic-land"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn reach_declare_attackers(game: &mut Game) {
    for _ in 0..4 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority passes reach attacker declaration");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

fn reach_declare_attackers_after_begin(game: &mut Game) {
    for _ in 0..8 {
        if game.step == Step::DeclareAttackers {
            return;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority passes reach attacker declaration after begin-game setup");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

#[test]
fn forestwalk_must_block_attacker_allows_empty_blockers_against_a_forest() {
    let attacker_controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new_with_basic_land_types(
        [
            creature(
                ATTACKER,
                vec![
                    Keyword::Haste,
                    Keyword::MustBeBlockedIfAble,
                    Keyword::Landwalk(BasicLandType::Forest),
                ],
            ),
            creature(BLOCKER, vec![]),
            forest(),
        ],
        2,
        [BasicLandTypeBinding {
            card_definition: FOREST,
            land_type: BasicLandType::Forest,
        }],
    )
    .expect("typed-land fixture initializes");
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker enters");
    game.put_on_battlefield(defender, BLOCKER)
        .expect("defender has a creature");
    game.put_on_battlefield(defender, FOREST)
        .expect("defender controls a Forest");
    game.turn = 2;

    reach_declare_attackers(&mut game);
    game.declare_attackers(attacker_controller, &[attacker])
        .expect("landwalk attacker is legal to declare");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes");
    game.pass_priority(defender)
        .expect("defender reaches blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);

    let result = game.declare_blockers(defender, &[]);
    eprintln!(
        "must-block landwalk red result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a Forest makes this attacker unblockable, so no creature is able to satisfy its must-block requirement"
    );
    game.validate_invariants()
        .expect("legal empty landwalk block declaration remains valid");
}

#[test]
fn must_block_scans_current_controller_not_owner_battlefield() {
    let attacker_controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new_with_basic_land_types(
        [
            creature(ATTACKER, vec![Keyword::Haste, Keyword::MustBeBlockedIfAble]),
            creature(BLOCKER, vec![]),
            forest(),
        ],
        2,
        [BasicLandTypeBinding {
            card_definition: FOREST,
            land_type: BasicLandType::Forest,
        }],
    )
    .expect("typed-land fixture initializes");
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker enters");
    let stolen_blocker = game
        .put_on_battlefield(attacker_controller, BLOCKER)
        .expect("attacker controller owns the eventual blocker");
    let control_source = game
        .put_on_battlefield(defender, FOREST)
        .expect("defender controls a live effect source");
    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        control_source,
        stolen_blocker,
        ContinuousChange::ChangeController(defender),
        Duration::Permanent,
    )
    .expect("defender gains control of the opponent-owned blocker");

    reach_declare_attackers_after_begin(&mut game);
    game.declare_attackers(attacker_controller, &[attacker])
        .expect("must-block attacker is legal to declare");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes");
    game.pass_priority(defender)
        .expect("defender reaches blocker declaration");

    let result = game.declare_blockers(defender, &[]);
    eprintln!(
        "must-block derived-control red result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction(
                "a must-be-blocked attacker had a legal unassigned blocker"
            ))
        ),
        "a defender-controlled opponent-owned creature must count as an available blocker"
    );
    game.validate_invariants()
        .expect("rejected empty assignment retains live control and combat state");
}
