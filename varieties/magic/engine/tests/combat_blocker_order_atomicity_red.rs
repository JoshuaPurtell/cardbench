//! Red regression: a rejected blocker declaration must not retain its combat prefix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, Game, ManaCost, PlayerId, RulesError, Step, Zone,
};

const ATTACKER: &str = "TST-BLOCKER-ORDER-ATOMIC-ATTACKER";
const BLOCKER: &str = "TST-BLOCKER-ORDER-ATOMIC-BLOCKER";

fn creature(id: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([color]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["test-only-combat-transaction"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_declare_attackers(game: &mut Game) {
    for _ in 0..4 {
        pass_pair(game);
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

#[test]
fn rejected_damage_order_decision_leaves_no_blocker_declaration_prefix() {
    // 256 distinct blockers are legal data but cannot fit the current u8
    // public combat-order decision. The range rejection happens only after
    // normal blocker validation used to commit the declaration.
    let blocker_count = usize::from(u8::MAX) + 1;
    let mut game = Game::new(
        [
            creature(ATTACKER, Color::Red),
            creature(BLOCKER, Color::Green),
        ],
        2,
    )
    .expect("fixture initializes");
    let attacker = game
        .add_card(PlayerId(0), ATTACKER, Zone::Battlefield)
        .expect("attacker enters before the game");
    let blockers = (0..blocker_count)
        .map(|_| {
            game.add_card(PlayerId(1), BLOCKER, Zone::Battlefield)
                .expect("blocker enters before the game")
        })
        .collect::<Vec<_>>();
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker predates turn one");
    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("the unblocked attack declaration succeeds");
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);

    let assignments = blockers
        .iter()
        .copied()
        .map(|blocker| CombatBlock { attacker, blocker })
        .collect::<Vec<_>>();
    let before_events = game.canonical_event_log();
    let result = game.declare_blockers(PlayerId(1), &assignments);
    let after_events = game.canonical_event_log();
    let last_event_is_blockers_declared = after_events
        .last()
        .is_some_and(|event| event.starts_with("BlockersDeclared"));
    eprintln!(
        "oversized combat-order declaration: {result:?}; blockers_declared={}; \\
         event_count={} last_event_is_blockers_declared={last_event_is_blockers_declared}",
        game.view_for_player(PlayerId(1))
            .expect("defender view remains available")
            .blockers_declared,
        after_events.len(),
    );
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "combat damage-order group exceeds engine range"
        ))
    ));
    assert!(
        !game
            .view_for_player(PlayerId(1))
            .expect("defender view remains available")
            .blockers_declared,
        "a rejected damage-order decision retained the blocker declaration"
    );
    assert_eq!(
        after_events, before_events,
        "a rejected blocker declaration retained visible combat events"
    );
    game.validate_invariants()
        .expect("the rejected declaration leaves an audit-valid pre-declaration state");
}
