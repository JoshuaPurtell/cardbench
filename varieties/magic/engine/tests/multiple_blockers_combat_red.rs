//! Red regression for ordinary multi-block combat declaration.
//!
//! The compact combat model must not reject a legal declaration merely because
//! two ordinary creatures block the same attacker.  This probe deliberately
//! stops at declaration: it exposes the missing game-state representation
//! before any damage-order or assignment implementation can hide it.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, Game, GameEvent, ManaCost, PlayerId, Step, Zone,
};

const LAND: &str = "TEST-LAND";
const ATTACKER: &str = "TEST-ATTACKER";
const BLOCKER: &str = "TEST-BLOCKER";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: LAND,
            name: "Test Land",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: colors([Color::Green]),
            card_types: types([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["basic-mana"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: ATTACKER,
            name: "Test Attacker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: BLOCKER,
            name: "Test Blocker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn add_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, LAND, Zone::Library)
            .expect("test library card is valid");
    }
}

fn advance_to_declare_attackers(game: &mut Game) {
    while game.turn != 3 || game.step != Step::DeclareAttackers {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("take ordinary draw");
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty declaration is legal");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
}

fn advance_to_blockers(game: &mut Game, attacker: cardbench_magic_engine::ObjectId) {
    advance_to_declare_attackers(game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker attacks");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player).expect("advance to blockers");
    }
    assert_eq!(game.step, Step::DeclareBlockers);
}

#[test]
fn ordinary_defender_can_assign_two_blockers_to_one_attacker_atomically() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    add_library(&mut game, PlayerId(0));
    add_library(&mut game, PlayerId(1));
    let attacker = game
        .put_on_battlefield(PlayerId(0), ATTACKER)
        .expect("attacker enters");
    let first_blocker = game
        .put_on_battlefield(PlayerId(1), BLOCKER)
        .expect("first blocker enters");
    let second_blocker = game
        .put_on_battlefield(PlayerId(1), BLOCKER)
        .expect("second blocker enters");

    game.begin_game().expect("game begins");
    advance_to_blockers(&mut game, attacker);
    game.clear_event_log();

    let result = game.declare_blockers(
        PlayerId(1),
        &[
            CombatBlock {
                attacker,
                blocker: first_blocker,
            },
            CombatBlock {
                attacker,
                blocker: second_blocker,
            },
        ],
    );

    assert_eq!(
        result,
        Ok(()),
        "two ordinary blockers on one attacker are legal; event log must remain atomic: {:#?}",
        game.event_log
    );
    assert_eq!(
        game.event_log,
        [GameEvent::BlockersDeclared {
            player: PlayerId(1),
            assignments: vec![(attacker, first_blocker), (attacker, second_blocker)],
        }],
        "successful multi-block declaration receipt"
    );
    game.validate_invariants()
        .expect("multi-block declaration preserves the game state machine");
}
