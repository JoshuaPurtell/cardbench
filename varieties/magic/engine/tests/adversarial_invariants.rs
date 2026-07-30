//! Adversarial public-API checks for rules boundaries and invariant preservation.
//!
//! The compact definitions here are intentionally expansion-neutral analogues of
//! the RAV fixture mechanisms. Keeping them in the engine crate avoids a test-only
//! dependency cycle from `cardbench-magic-rav` back to this engine.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ContinuousChange, Duration, Effect, Game,
    GameEvent, Keyword, ManaCost, ObjectId, PlayerId, PolicyAction, PolicyMoveKind, RulesError,
    Target, TargetRequirement, Zone,
};

const RED_BOLT: &str = "TEST-RED-BOLT";
const PING: &str = "TEST-PING";
const BIGGER_PING: &str = "TEST-BIGGER-PING";
const SHRINK: &str = "TEST-SHRINK";
const SOURCE: &str = "TEST-SOURCE";
const BODY: &str = "TEST-BODY";
const TRANSMUTER: &str = "TEST-TRANSMUTER";
const TUTOR_TARGET: &str = "TEST-TUTOR-TARGET";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn instant(id: &'static str, mana_cost: ManaCost, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["test-effect"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn creature(id: &'static str, power: i16, toughness: i16) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: colors([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(power),
        toughness: Some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        instant(
            RED_BOLT,
            ManaCost::with_colors(0, [Color::Red]),
            vec![Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::Player,
            }],
        ),
        instant(
            PING,
            ManaCost::new(0),
            vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
        ),
        instant(
            BIGGER_PING,
            ManaCost::new(0),
            vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::Player,
            }],
        ),
        instant(
            SHRINK,
            ManaCost::new(0),
            vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -1,
                toughness: -1,
            }],
        ),
        creature(SOURCE, 2, 2),
        creature(BODY, 1, 1),
        CardDefinition {
            id: TRANSMUTER,
            name: TRANSMUTER,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["transmute"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![],
        },
        instant(TUTOR_TARGET, ManaCost::new(2), vec![]),
    ]
}

fn assert_invariants(game: &Game) {
    game.validate_invariants()
        .expect("each publicly reachable state must satisfy the invariant audit");
}

fn pass_all_survivors(game: &mut Game) {
    if game.step == cardbench_magic_engine::Step::DeclareAttackers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .attackers_declared
    {
        game.declare_attackers(game.next_policy_player(), &[])
            .expect("empty attackers are explicit");
    }
    if game.step == cardbench_magic_engine::Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .blockers_declared
    {
        game.declare_blockers(game.next_policy_player(), &[])
            .expect("empty blockers are explicit");
    }
    let passes = game.players.iter().filter(|player| !player.lost).count();
    for _ in 0..passes {
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("take the ordinary draw");
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("the current living priority holder can pass");
        assert_invariants(game);
    }
}

fn cast(game: &mut Game, player: PlayerId, card: ObjectId, target: Target) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets: vec![target],
            convoke: vec![],
        },
    )
    .expect("test card is legal to cast");
    assert_invariants(game);
}

#[test]
fn rejected_cast_is_atomic_and_does_not_contaminate_the_event_log() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    let card = game
        .add_card(player, RED_BOLT, Zone::Hand)
        .expect("known test card enters hand");
    assert_invariants(&game);

    let before_hand = game.player(player).expect("player exists").hand.clone();
    let before_stack = game.stack.clone();
    let before_events = game.event_log.clone();
    let before_object = game.object(card).expect("card exists").clone();
    let error = game
        .cast_spell(
            player,
            CastRequest {
                card,
                targets: vec![Target::Player(opponent)],
                convoke: vec![],
            },
        )
        .expect_err("a red spell cannot be cast from an empty mana pool");
    assert!(matches!(error, RulesError::Mana(_)));

    assert_eq!(
        game.player(player).expect("player exists").hand,
        before_hand
    );
    assert_eq!(game.stack, before_stack);
    assert_eq!(game.event_log, before_events);
    assert_eq!(game.object(card).expect("card exists"), &before_object);
    assert_eq!(game.zone_of(card), Some(Zone::Hand));
    assert_invariants(&game);
}

#[test]
fn policy_submitted_transmute_uses_the_audited_engine_path() {
    let player = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    let transmuter = game
        .add_card(player, TRANSMUTER, Zone::Hand)
        .expect("transmute card enters hand");
    let found = game
        .add_card(player, TUTOR_TARGET, Zone::Library)
        .expect("matching mana-value card enters library");
    let opponent_library_card = game
        .add_card(PlayerId(1), TUTOR_TARGET, Zone::Library)
        .expect("opponent matching card remains private");
    game.grant_mana(player, Color::Blue, 2)
        .expect("setup blue mana");
    game.grant_mana(player, Color::Green, 1)
        .expect("setup generic payment mana");

    let view = game
        .view_for_player(player)
        .expect("controller receives transmute search choices");
    let search = view
        .transmute_searches
        .iter()
        .find(|search| search.card == transmuter)
        .expect("transmute card has a candidate projection");
    assert_eq!(search.candidates.len(), 1);
    assert_eq!(search.candidates[0].id, found);
    assert_ne!(search.candidates[0].id, opponent_library_card);

    game.submit_policy_move(
        player,
        "engine-transmute-contract",
        PolicyAction::Transmute {
            card: transmuter,
            found: Some(search.candidates[0].id),
        },
    )
    .expect("legal transmute is accepted through policy submission");

    assert_eq!(game.zone_of(transmuter), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(found), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::Transmuted { discarded, found: selected, .. }
                if *discarded == transmuter && *selected == Some(found)
        )
    }));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::PolicyMoveSubmitted {
                kind: PolicyMoveKind::Transmute,
                ..
            }
        )
    }));
    assert_invariants(&game);
}

#[test]
fn mana_is_cleared_at_the_next_step_boundary_without_losing_invariants() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    game.add_mana_from_action(player, Color::Red, 3)
        .expect("priority holder adds mana through the public action path");
    game.grant_mana(opponent, Color::Blue, 2)
        .expect("scenario setup grants opponent floating mana");
    assert_invariants(&game);

    pass_all_survivors(&mut game);
    assert_eq!(game.step, cardbench_magic_engine::Step::BeginningOfCombat);
    for player in [player, opponent] {
        assert_eq!(
            game.player(player)
                .expect("player exists")
                .mana_pool
                .total(),
            0,
            "phase transitions clear every player's floating mana"
        );
    }
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::ManaAdded {
                player: mana_player,
                color: Color::Red,
                amount: 3,
            } if *mana_player == player
        )
    }));
    assert_invariants(&game);
}

#[test]
fn stack_is_lifo_and_a_target_that_left_play_is_countered_by_rules() {
    let first_player = PlayerId(0);
    let second_player = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    let first_spell = game
        .add_card(first_player, PING, Zone::Hand)
        .expect("first spell enters hand");
    let second_spell = game
        .add_card(second_player, BIGGER_PING, Zone::Hand)
        .expect("second spell enters hand");

    cast(
        &mut game,
        first_player,
        first_spell,
        Target::Player(second_player),
    );
    game.pass_priority(first_player)
        .expect("the caster passes before the opponent responds");
    cast(
        &mut game,
        second_player,
        second_spell,
        Target::Player(first_player),
    );
    assert_eq!(game.stack.len(), 2);
    pass_all_survivors(&mut game);
    assert_eq!(game.stack.len(), 1, "only the top stack object resolves");
    pass_all_survivors(&mut game);
    assert!(game.stack.is_empty());
    assert_eq!(game.player(first_player).expect("player exists").life, 18);
    assert_eq!(game.player(second_player).expect("player exists").life, 19);
    let resolved: Vec<_> = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellResolved { card } => Some(*card),
            _ => None,
        })
        .collect();
    assert_eq!(resolved, vec![second_spell, first_spell]);
    assert_invariants(&game);

    let mut target_game = Game::new(definitions(), 2).expect("second game initializes");
    let shrink = target_game
        .add_card(first_player, SHRINK, Zone::Hand)
        .expect("shrink spell enters hand");
    let source = target_game
        .put_on_battlefield(first_player, SOURCE)
        .expect("continuous-effect source enters battlefield");
    let target = target_game
        .put_on_battlefield(second_player, BODY)
        .expect("one-toughness target enters battlefield");
    cast(
        &mut target_game,
        first_player,
        shrink,
        Target::Permanent(target),
    );
    target_game
        .add_continuous_effect(
            source,
            target,
            ContinuousChange::ModifyPowerToughness {
                power: -1,
                toughness: -1,
            },
            Duration::Permanent,
        )
        .expect("public continuous effect installs");
    target_game
        .check_state_based_actions()
        .expect("zero-toughness target is processed by SBAs");
    assert_eq!(target_game.zone_of(target), Some(Zone::Graveyard));
    assert_invariants(&target_game);

    pass_all_survivors(&mut target_game);
    assert_eq!(target_game.zone_of(shrink), Some(Zone::Graveyard));
    assert!(target_game.event_log.iter().any(|event| {
        matches!(event, GameEvent::SpellCounteredByRules { card } if *card == shrink)
    }));
    assert!(
        !target_game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == shrink))
    );
    assert_invariants(&target_game);
}

#[test]
fn state_based_actions_reach_a_fixed_point_for_multiple_lethal_creatures() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    let source = game
        .put_on_battlefield(controller, SOURCE)
        .expect("continuous-effect source enters battlefield");
    let first = game
        .put_on_battlefield(opponent, BODY)
        .expect("first body enters battlefield");
    let second = game
        .put_on_battlefield(opponent, BODY)
        .expect("second body enters battlefield");
    for target in [first, second] {
        game.add_continuous_effect(
            source,
            target,
            ContinuousChange::ModifyPowerToughness {
                power: -1,
                toughness: -1,
            },
            Duration::Permanent,
        )
        .expect("lethal layer effect is installed");
    }
    assert_invariants(&game);

    game.check_state_based_actions()
        .expect("SBA loop reaches a stable state");
    assert_eq!(game.zone_of(first), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(second), Some(Zone::Graveyard));
    let sba_count = game
        .event_log
        .iter()
        .filter(|event| matches!(event, GameEvent::StateBasedAction { .. }))
        .count();
    assert_eq!(sba_count, 2);
    let event_count_at_fixed_point = game.event_log.len();
    game.check_state_based_actions()
        .expect("a fixed point can be checked repeatedly");
    assert_eq!(game.event_log.len(), event_count_at_fixed_point);
    assert_invariants(&game);
}

#[test]
fn multiplayer_priority_skips_eliminated_players_and_ends_with_one_survivor() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let third = PlayerId(2);
    let mut game = Game::new(definitions(), 3).expect("three-player game initializes");

    game.draw_card(first, None)
        .expect("drawing from an empty library applies the loss rule");
    assert!(game.player(first).expect("first player exists").lost);
    assert!(!game.is_game_over());
    assert_eq!(game.priority, second);
    assert_eq!(game.next_policy_player(), second);
    assert_invariants(&game);
    assert!(matches!(
        game.pass_priority(first),
        Err(RulesError::IllegalAction("an eliminated player cannot act"))
    ));
    assert_invariants(&game);

    game.draw_card(second, None)
        .expect("second empty draw leaves one survivor");
    assert!(game.is_game_over());
    assert_eq!(game.winner(), Some(third));
    // Once only one player remains, the game is terminal and no player receives
    // another priority decision. The non-terminal assertions above verify that
    // priority skipped the first eliminated player.
    assert_invariants(&game);
}

#[test]
fn simultaneous_player_losses_end_without_a_winner_or_a_panic() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    for player in &mut game.players {
        player.life = 0;
    }

    game.check_state_based_actions()
        .expect("simultaneous state-based losses are processed");

    assert!(game.is_game_over());
    assert_eq!(game.winner(), None);
    assert!(game.players.iter().all(|player| player.lost));
    assert_invariants(&game);
}
