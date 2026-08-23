//! Red regression for trample combat-damage assignment.
//!
//! The test intentionally runs with the keyword represented but without any
//! special combat implementation. That makes the missing excess-to-player
//! assignment an observable engine defect rather than a card-data claim.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, Effect, Game, GameEvent, Keyword,
    ManaCost, PlayerId, Step, Target, TargetRequirement, Zone,
};

const LAND: &str = "TEST-LAND";
const TRAMPLER: &str = "TEST-TRAMPLER";
const BLOCKER: &str = "TEST-BLOCKER";
const PING: &str = "TEST-PING";
const REMOVAL: &str = "TEST-REMOVAL";

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
            id: PING,
            name: "Test Ping",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["targeted-damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Creature,
            }],
        },
        CardDefinition {
            id: REMOVAL,
            name: "Test Removal",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["targeted-damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::Creature,
            }],
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

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    while game.turn != turn || game.step != step {
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

fn advance_to_declare_attackers(game: &mut Game) {
    advance_to(game, 3, Step::DeclareAttackers);
}

fn resolve_top(game: &mut Game) {
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("spell resolves after passes");
    }
}

fn advance_to_blockers(game: &mut Game, trampler: cardbench_magic_engine::ObjectId) {
    advance_to_declare_attackers(game);
    game.declare_attackers(PlayerId(0), &[trampler])
        .expect("trampler attacks");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player).expect("advance to blockers");
    }
    assert_eq!(game.step, Step::DeclareBlockers);
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

    game.begin_game().expect("game begins");
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

#[test]
fn trample_uses_previously_marked_damage_when_calculating_lethal_assignment() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    add_library(&mut game, PlayerId(0));
    add_library(&mut game, PlayerId(1));
    let trampler = game
        .put_on_battlefield(PlayerId(0), TRAMPLER)
        .expect("trampler enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), BLOCKER)
        .expect("blocker enters");
    let ping = game
        .add_card(PlayerId(0), PING, Zone::Hand)
        .expect("ping enters hand");

    game.begin_game().expect("game begins");
    advance_to(&mut game, 3, Step::PrecombatMain);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: ping,
            targets: vec![Target::Permanent(blocker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("ping casts");
    resolve_top(&mut game);
    advance_to_blockers(&mut game, trampler);
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: trampler,
            blocker,
        }],
    )
    .expect("single block is legal");
    resolve_top(&mut game);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 }
            if *source == trampler && *permanent == blocker
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 }
            if *source == trampler
    )));
    assert_eq!(game.player(PlayerId(1)).expect("defender exists").life, 16);
    game.validate_invariants().expect("state stays valid");
}

#[test]
fn trample_assigns_all_positive_damage_to_defender_when_sole_blocker_leaves_combat() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    add_library(&mut game, PlayerId(0));
    add_library(&mut game, PlayerId(1));
    let trampler = game
        .put_on_battlefield(PlayerId(0), TRAMPLER)
        .expect("trampler enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), BLOCKER)
        .expect("blocker enters");
    let removal = game
        .add_card(PlayerId(0), REMOVAL, Zone::Hand)
        .expect("removal enters hand");

    game.begin_game().expect("game begins");
    advance_to_blockers(&mut game, trampler);
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: trampler,
            blocker,
        }],
    )
    .expect("single block is legal");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(blocker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("removal casts after blockers");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(blocker), Some(Zone::Graveyard));
    resolve_top(&mut game);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 5 }
            if *source == trampler
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, .. }
            if *source == trampler && *permanent == blocker
    )));
    assert_eq!(game.player(PlayerId(1)).expect("defender exists").life, 15);
    game.validate_invariants().expect("state stays valid");
}
