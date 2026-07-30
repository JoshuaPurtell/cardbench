//! Zone, token, target, and state-based-action transition contracts.
//!
//! These are public-API tests for the current expansion-neutral engine slice.
//! Each accepted transition is immediately audited with `validate_invariants`;
//! each rejected transition must leave all observable game state unchanged.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ContinuousChange, Duration, Effect, Game,
    GameEvent, ManaCost, ObjectId, PlayerId, RulesError, Step, Target, TargetRequirement,
    TokenSpec, Zone,
};

const PERMANENT: &str = "ZONE-PERMANENT";
const SOURCE: &str = "ZONE-SOURCE";
const FRAGILE: &str = "ZONE-FRAGILE";
const PING: &str = "ZONE-PING";
const KILL: &str = "ZONE-KILL";
const TOKEN_MAKER: &str = "ZONE-TOKEN-MAKER";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
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

fn instant(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
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

fn definitions() -> Vec<CardDefinition> {
    vec![
        creature(PERMANENT, 2, 2),
        creature(SOURCE, 2, 2),
        creature(FRAGILE, 1, 1),
        instant(
            PING,
            vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
        ),
        instant(
            KILL,
            vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Creature,
            }],
        ),
        instant(
            TOKEN_MAKER,
            vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 1,
            }],
        ),
    ]
}

fn game(player_count: usize) -> Game {
    Game::new(definitions(), player_count).expect("test game initializes")
}

fn assert_invariants(game: &Game) {
    game.validate_invariants()
        .expect("every publicly-reachable state satisfies the invariant contract");
}

fn pass_all_survivors(game: &mut Game) {
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .attackers_declared
    {
        game.declare_attackers(game.next_policy_player(), &[])
            .expect("empty attackers are explicit");
    }
    if game.step == Step::DeclareBlockers
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

fn cast(game: &mut Game, player: PlayerId, card: ObjectId, targets: Vec<Target>) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets,
            convoke: vec![],
        },
    )
    .expect("the test spell is legal to cast");
    assert_invariants(game);
}

fn zone_memberships(game: &Game, card: ObjectId) -> Vec<Zone> {
    game.players
        .iter()
        .flat_map(|player| {
            [
                (Zone::Library, &player.library),
                (Zone::Hand, &player.hand),
                (Zone::Battlefield, &player.battlefield),
                (Zone::Graveyard, &player.graveyard),
                (Zone::Exile, &player.exile),
            ]
        })
        .filter_map(|(zone, cards)| cards.contains(&card).then_some(zone))
        .collect()
}

#[test]
fn begin_game_completes_automatic_untap_and_returns_at_upkeep_priority() {
    let first = PlayerId(0);
    let mut game = game(2);

    game.begin_game()
        .expect("a prepared game starts through its automatic untap transition");

    assert_eq!(game.turn, 1);
    assert_eq!(game.active_player, first);
    assert_eq!(game.step, Step::Upkeep);
    assert_eq!(game.priority, first);
    assert_eq!(
        game.event_log
            .iter()
            .filter_map(|event| match event {
                GameEvent::StepBegan { step, .. } => Some(*step),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![Step::Untap, Step::Upkeep],
        "the log preserves the automatic no-priority step before the first priority window"
    );
    assert_invariants(&game);
}

#[test]
fn first_turn_draw_is_skipped_only_in_two_player_games() {
    let first = PlayerId(0);
    let mut game = game(3);
    let draw = game
        .add_card(first, PING, Zone::Library)
        .expect("starting player has one library card");

    game.begin_game()
        .expect("three-player game reaches its first upkeep");
    pass_all_survivors(&mut game);

    assert_eq!(game.step, Step::Draw);
    assert!(
        game.view_for_player(first)
            .expect("starting player draw view")
            .draw_replacement_pending,
        "the multiplayer starting player receives a draw-replacement decision"
    );
    game.resolve_pending_draw(first, None)
        .expect("the starting player takes the ordinary draw");
    assert_eq!(
        game.zone_of(draw),
        Some(Zone::Hand),
        "the starting player draws on turn one in a multiplayer game"
    );
    assert!(game.event_log.iter().any(
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == draw)
    ));
    assert_invariants(&game);
}

#[test]
fn permanent_spell_moves_hand_to_stack_to_battlefield_exactly_once() {
    let controller = PlayerId(0);
    let mut game = game(2);
    let permanent = game
        .add_card(controller, PERMANENT, Zone::Hand)
        .expect("permanent starts in its owner's hand");
    assert_eq!(zone_memberships(&game, permanent), vec![Zone::Hand]);
    assert_invariants(&game);

    cast(&mut game, controller, permanent, vec![]);
    assert_eq!(game.zone_of(permanent), None);
    assert!(zone_memberships(&game, permanent).is_empty());
    assert_eq!(game.stack.len(), 1);
    assert_eq!(game.stack[0].card, permanent);
    assert_invariants(&game);

    pass_all_survivors(&mut game);
    assert!(game.stack.is_empty());
    assert_eq!(game.zone_of(permanent), Some(Zone::Battlefield));
    assert_eq!(zone_memberships(&game, permanent), vec![Zone::Battlefield]);
    assert_eq!(
        game.object(permanent)
            .expect("resolved permanent still exists")
            .owner,
        controller
    );
    assert_eq!(
        game.object(permanent)
            .expect("resolved permanent still exists")
            .controller,
        controller
    );
    assert!(
        game.event_log
            .iter()
            .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == permanent))
    );
    assert!(game.event_log.iter().any(
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == permanent)
    ));
    assert_invariants(&game);
}

#[test]
fn rejected_attempt_to_cast_an_opponents_hand_card_is_atomic() {
    let owner = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game(2);
    let legal_spell = game
        .add_card(owner, PING, Zone::Hand)
        .expect("owner has a legal spell");
    let protected_card = game
        .add_card(owner, PING, Zone::Hand)
        .expect("owner has another hand card");

    cast(
        &mut game,
        owner,
        legal_spell,
        vec![Target::Player(opponent)],
    );
    assert_eq!(game.priority, opponent);
    let before_players = game.players.clone();
    let before_stack = game.stack.clone();
    let before_events = game.event_log.clone();

    assert!(matches!(
        game.cast_spell(
            opponent,
            CastRequest {
                card: protected_card,
                targets: vec![Target::Player(owner)],
                convoke: vec![],
            },
        ),
        Err(RulesError::IllegalAction(
            "only your own hand card may be cast"
        ))
    ));

    assert_eq!(game.players, before_players);
    assert_eq!(game.stack, before_stack);
    assert_eq!(game.event_log, before_events);
    assert_eq!(game.zone_of(protected_card), Some(Zone::Hand));
    assert_eq!(zone_memberships(&game, protected_card), vec![Zone::Hand]);
    assert_invariants(&game);
}

#[test]
fn target_leaving_the_battlefield_counters_the_pending_spell_and_preserves_zone_integrity() {
    let caster = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = game(2);
    let removal = game
        .add_card(caster, KILL, Zone::Hand)
        .expect("targeted spell enters hand");
    let source = game
        .put_on_battlefield(caster, SOURCE)
        .expect("continuous-effect source enters battlefield");
    let target = game
        .put_on_battlefield(defender, FRAGILE)
        .expect("one-toughness target enters battlefield");

    cast(&mut game, caster, removal, vec![Target::Permanent(target)]);
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ModifyPowerToughness {
            power: -1,
            toughness: -1,
        },
        Duration::Permanent,
    )
    .expect("battlefield effect makes the target state-based-action lethal");
    game.check_state_based_actions()
        .expect("state-based actions move the zero-toughness target");
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(zone_memberships(&game, target), vec![Zone::Graveyard]);
    let moved_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if *card == target
            )
        })
        .expect("target zone change is logged");
    let expired_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::ContinuousEffectExpired {
                    source: logged_source,
                    target: logged_target,
                    ..
                } if *logged_source == source && *logged_target == target
            )
        })
        .expect("zone departure explicitly expires the continuous effect");
    assert!(moved_index < expired_index);
    assert_invariants(&game);

    pass_all_survivors(&mut game);
    assert!(game.stack.is_empty());
    assert_eq!(game.zone_of(removal), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(
        |event| matches!(event, GameEvent::SpellCounteredByRules { card } if *card == removal)
    ));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == removal)),
        "a spell with its only target gone must never resolve a partial effect"
    );
    assert_invariants(&game);
}

#[test]
fn token_sba_emits_cease_event_and_removes_all_object_and_zone_references() {
    let controller = PlayerId(0);
    let mut game = game(2);
    let source = game
        .put_on_battlefield(controller, SOURCE)
        .expect("source enters battlefield");
    let maker = game
        .add_card(controller, TOKEN_MAKER, Zone::Hand)
        .expect("token maker enters hand");

    cast(&mut game, controller, maker, vec![]);
    pass_all_survivors(&mut game);
    let token = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { token, .. } => Some(*token),
            _ => None,
        })
        .expect("the resolved spell created one token");
    assert_eq!(game.zone_of(token), Some(Zone::Battlefield));
    assert_eq!(zone_memberships(&game, token), vec![Zone::Battlefield]);

    game.add_continuous_effect(
        source,
        token,
        ContinuousChange::ModifyPowerToughness {
            power: -1,
            toughness: -1,
        },
        Duration::Permanent,
    )
    .expect("a public continuous effect can make the token lethal");
    game.check_state_based_actions()
        .expect("state-based actions process the lethal token");

    assert_eq!(game.zone_of(token), None);
    assert!(zone_memberships(&game, token).is_empty());
    assert!(matches!(game.object(token), Err(RulesError::UnknownCard(card)) if card == token));
    let sba_index = game
        .event_log
        .iter()
        .position(
            |event| matches!(event, GameEvent::StateBasedAction { card, .. } if *card == token),
        )
        .expect("token death has an SBA receipt");
    let ceased_index = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::TokenCeasedToExist { token: ceased } if *ceased == token))
        .expect("token disappearance has an explicit lifecycle receipt");
    assert!(sba_index < ceased_index);
    assert_invariants(&game);
}

#[test]
fn losing_multiplayer_player_removes_owned_objects_and_records_each_exit() {
    let eliminated = PlayerId(0);
    let first_survivor = PlayerId(1);
    let second_survivor = PlayerId(2);
    let mut game = game(3);
    let hand = game
        .add_card(eliminated, PING, Zone::Hand)
        .expect("losing player owns a hand card");
    let library = game
        .add_card(eliminated, PING, Zone::Library)
        .expect("losing player owns a library card");
    let battlefield = game
        .put_on_battlefield(eliminated, PERMANENT)
        .expect("losing player owns a permanent");
    let stack_spell = game
        .add_card(eliminated, PING, Zone::Hand)
        .expect("losing player owns a spell that is later on the stack");
    let survivor_permanent = game
        .put_on_battlefield(first_survivor, PERMANENT)
        .expect("surviving player has an unrelated permanent");
    assert_invariants(&game);
    cast(
        &mut game,
        eliminated,
        stack_spell,
        vec![Target::Player(first_survivor)],
    );
    assert_eq!(game.priority, first_survivor);
    assert_eq!(game.stack.len(), 1);

    // This setup represents the state immediately before the next priority
    // window after lethal damage.  The public SBA entry point must process the
    // player loss and the immediate multiplayer leave-game cleanup together.
    game.players[eliminated.0].life = 0;
    game.check_state_based_actions()
        .expect("zero life marks the player as lost and cleans up owned objects");

    assert!(game.player(eliminated).expect("seat persists").lost);
    assert!(!game.player(first_survivor).expect("seat persists").lost);
    assert!(!game.player(second_survivor).expect("seat persists").lost);
    for object in [hand, library, battlefield, stack_spell] {
        assert_eq!(game.zone_of(object), None);
        assert!(zone_memberships(&game, object).is_empty());
        assert!(
            matches!(game.object(object), Err(RulesError::UnknownCard(card)) if card == object)
        );
        assert!(game.event_log.iter().any(|event| {
            matches!(
                event,
                GameEvent::ObjectLeftGame { object: departed, owner }
                    if *departed == object && *owner == eliminated
            )
        }));
    }
    assert!(game.stack.is_empty());
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::GameEnded { .. })),
        "one loss in a three-player game is nonterminal"
    );
    assert_eq!(
        game.zone_of(survivor_permanent),
        Some(Zone::Battlefield),
        "a player leaving must not remove objects owned by remaining players"
    );
    assert!(!game.is_game_over());
    assert_eq!(game.priority, first_survivor);
    assert_invariants(&game);
}
