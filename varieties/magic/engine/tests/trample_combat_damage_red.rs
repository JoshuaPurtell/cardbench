//! Red regression for trample combat-damage assignment.
//!
//! The test intentionally runs with the keyword represented but without any
//! special combat implementation. That makes the missing excess-to-player
//! assignment an observable engine defect rather than a card-data claim.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, Game, GameEvent, Keyword, ManaCost, PlayerId,
    Step, Zone,
};

const LAND: &str = "TEST-LAND";
const TRAMPLER: &str = "TEST-TRAMPLER";
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
            id: TRAMPLER,
            name: "Test Trampler",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["trample", "base-characteristics"],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![Keyword::Trample],
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
    game.begin_game().expect("game begins");
    while !(game.turn == 3 && game.step == Step::DeclareAttackers) {
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
            Step::DeclareAttackers => {
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .attackers_declared
                {
                    let player = game.next_policy_player();
                    game.declare_attackers(player, &[])
                        .expect("empty declaration is legal");
                } else {
                    let player = game.priority;
                    game.pass_priority(player).expect("priority passes");
                }
            }
            Step::DeclareBlockers => {
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .blockers_declared
                {
                    let player = game.next_policy_player();
                    game.declare_blockers(player, &[])
                        .expect("empty declaration is legal");
                } else {
                    let player = game.priority;
                    game.pass_priority(player).expect("priority passes");
                }
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
}

#[test]
fn trample_assigns_only_lethal_damage_to_a_single_blocker_and_excess_to_defender() {
    let attacker_player = PlayerId(0);
    let defending_player = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    add_library(&mut game, attacker_player);
    add_library(&mut game, defending_player);
    let trampler = game
        .put_on_battlefield(attacker_player, TRAMPLER)
        .expect("trampler enters");
    let blocker = game
        .put_on_battlefield(defending_player, BLOCKER)
        .expect("blocker enters");

    advance_to_declare_attackers(&mut game);
    game.declare_attackers(attacker_player, &[trampler])
        .expect("trampler attacks");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player).expect("advance to blockers");
    }
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(
        defending_player,
        &[CombatBlock {
            attacker: trampler,
            blocker,
        }],
    )
    .expect("single block is legal");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("advance to combat damage");
    }

    let trample_damage = game
        .event_log
        .iter()
        .filter(|event| matches!(event, GameEvent::DamageDealtToPlayer { source, player, amount: 3 } if *source == trampler && *player == defending_player))
        .count();
    assert_eq!(trample_damage, 1, "event log: {:#?}", game.event_log);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 }
            if *source == trampler && *permanent == blocker
    )));
    assert_eq!(
        game.player(defending_player).expect("defender exists").life,
        17
    );
    game.validate_invariants()
        .expect("trample damage transition remains valid");
}
