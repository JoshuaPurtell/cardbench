//! Deterministic stateful, property-style policy submission campaign.
//!
//! This is deliberately not a unit test of private helpers.  It repeatedly drives
//! the public `Game::submit_policy_move` boundary using a reproducible local PRNG,
//! and checks the public invariant audit after *every* accepted or rejected
//! proposal.  A failing assertion prints the seed, operation number, action, and
//! canonical event transcript so the exact trace is replayable.
//!
//! The fixture covers the engine mechanisms currently needed by the initial RAV
//! slice: priority, main phases and land plays, mana abilities, stack responses,
//! tokens, convoke, radiance-like continuous effects, SBAs, combat, dredge,
//! transmute, and three-player priority/elimination-safe rotation.

use std::collections::{BTreeSet, HashSet};

use cardbench_magic_engine::{
    CardDefinition, CardObject, CardType, CastRequest, Color, CombatBlock, ConvokeContribution,
    ConvokePayment, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ObjectId, PlayerId,
    PlayerState, PolicyAction, StackObject, Target, TargetRequirement, Zone,
};

const POLICY_ID: &str = "engine.stateful-policy-fuzz.v1";

const FOREST: &str = "FUZZ-FOREST";
const ISLAND: &str = "FUZZ-ISLAND";
const MOUNTAIN: &str = "FUZZ-MOUNTAIN";
const PLAINS: &str = "FUZZ-PLAINS";
const SWAMP: &str = "FUZZ-SWAMP";
const LIBRARY_FILLER: &str = "FUZZ-LIBRARY-FILLER";
const GREEN_BODY: &str = "FUZZ-GREEN-BODY";
const RED_BODY: &str = "FUZZ-RED-BODY";
const FRAGILE_BODY: &str = "FUZZ-FRAGILE-BODY";
const PING: &str = "FUZZ-PING";
const TOKEN_MAKER: &str = "FUZZ-TOKEN-MAKER";
const CONVOKE_RITUAL: &str = "FUZZ-CONVOKE-RITUAL";
const RADIANCE: &str = "FUZZ-RADIANCE";
const SHRINK: &str = "FUZZ-SHRINK";
const DREDGER: &str = "FUZZ-DREDGER";
const TRANSMUTER: &str = "FUZZ-TRANSMUTER";
const TRANSMUTE_TARGET: &str = "FUZZ-TRANSMUTE-TARGET";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn land(id: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "FZZ",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: colors([color]),
        card_types: types([CardType::Land]),
        is_basic_land: true,
        supported_rules: &["basic-mana"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn creature(id: &'static str, color: Color, power: i16, toughness: i16) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "FZZ",
        mana_cost: ManaCost::new(0),
        colors: colors([color]),
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

fn instant(id: &'static str, mana_cost: ManaCost, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "FZZ",
        mana_cost,
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["stateful-test-effect"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[allow(clippy::too_many_lines)] // Compact executable catalog keeps the campaign self-contained.
fn definitions() -> Vec<CardDefinition> {
    vec![
        land(FOREST, Color::Green),
        land(ISLAND, Color::Blue),
        land(MOUNTAIN, Color::Red),
        land(PLAINS, Color::White),
        land(SWAMP, Color::Black),
        creature(LIBRARY_FILLER, Color::Green, 1, 1),
        creature(GREEN_BODY, Color::Green, 2, 2),
        creature(RED_BODY, Color::Red, 2, 2),
        creature(FRAGILE_BODY, Color::White, 1, 1),
        instant(
            PING,
            ManaCost::new(0),
            vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
        ),
        instant(
            TOKEN_MAKER,
            ManaCost::new(0),
            vec![Effect::CreateToken {
                token: cardbench_magic_engine::TokenSpec::saproling(),
                count: 2,
            }],
        ),
        CardDefinition {
            id: CONVOKE_RITUAL,
            name: CONVOKE_RITUAL,
            set_code: "FZZ",
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["convoke", "create-token"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::CreateToken {
                token: cardbench_magic_engine::TokenSpec::saproling(),
                count: 1,
            }],
        },
        instant(
            RADIANCE,
            ManaCost::new(0),
            vec![Effect::RadianceUntapAndModifyUntilEndOfTurn {
                power: 1,
                toughness: 1,
            }],
        ),
        instant(
            SHRINK,
            ManaCost::new(0),
            vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -2,
                toughness: -2,
            }],
        ),
        CardDefinition {
            id: DREDGER,
            name: DREDGER,
            set_code: "FZZ",
            mana_cost: ManaCost::new(1),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["dredge"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Dredge(2)],
            effects: vec![],
        },
        CardDefinition {
            id: TRANSMUTER,
            name: TRANSMUTER,
            set_code: "FZZ",
            mana_cost: ManaCost::new(2),
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
        instant(
            TRANSMUTE_TARGET,
            ManaCost::new(2),
            vec![Effect::CreateToken {
                token: cardbench_magic_engine::TokenSpec::saproling(),
                count: 1,
            }],
        ),
    ]
}

/// Captures every mutable state element exposed by the public engine API.  The
/// rejected-operation property uses this to prevent a failed policy request from
/// becoming a hidden partial transition.
#[derive(Debug, Eq, PartialEq)]
struct ObservableState {
    players: Vec<PlayerState>,
    stack: Vec<StackObject>,
    continuous_effects: Vec<cardbench_magic_engine::ContinuousEffect>,
    active_player: PlayerId,
    priority: PlayerId,
    step: cardbench_magic_engine::Step,
    turn: u32,
    events: Vec<String>,
    views: Vec<GameView>,
    objects: Vec<CardObject>,
}

impl ObservableState {
    fn capture(game: &Game) -> Self {
        let mut object_ids = BTreeSet::new();
        for player in &game.players {
            object_ids.extend(player.library.iter().copied());
            object_ids.extend(player.hand.iter().copied());
            object_ids.extend(player.battlefield.iter().copied());
            object_ids.extend(player.graveyard.iter().copied());
            object_ids.extend(player.exile.iter().copied());
        }
        object_ids.extend(game.stack.iter().map(|object| object.card));
        let objects = object_ids
            .into_iter()
            .map(|card| {
                game.object(card)
                    .unwrap_or_else(|error| {
                        panic!("observable object {card:?} disappeared: {error}")
                    })
                    .clone()
            })
            .collect();
        let views = game
            .players
            .iter()
            .map(|player| {
                game.view_for_player(player.id).unwrap_or_else(|error| {
                    panic!(
                        "view for player {} failed while snapshotting: {error}",
                        player.id.0
                    )
                })
            })
            .collect();
        Self {
            players: game.players.clone(),
            stack: game.stack.clone(),
            continuous_effects: game.continuous_effects.clone(),
            active_player: game.active_player,
            priority: game.priority,
            step: game.step,
            turn: game.turn,
            events: game.canonical_event_log(),
            views,
            objects,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TraceRng {
    state: u64,
}

impl TraceRng {
    const fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0xD1B5_4A32_D192_ED03,
        }
    }

    fn choose(&mut self, upper_bound: usize) -> usize {
        assert!(
            upper_bound > 0,
            "a trace must not choose from an empty action set"
        );
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.state >> 32) as usize) % upper_bound
    }
}

#[derive(Debug, Eq, PartialEq)]
struct TraceReceipt {
    seed: u64,
    accepted: usize,
    rejected: usize,
    events: Vec<String>,
    final_turn: u32,
    final_step: cardbench_magic_engine::Step,
    final_life: Vec<i64>,
}

#[derive(Debug)]
struct Fixture {
    game: Game,
    forest: ObjectId,
    second_land: ObjectId,
    token_maker: ObjectId,
    convoke_ritual: ObjectId,
    radiance: ObjectId,
    shrink: ObjectId,
    transmuter: ObjectId,
    transmute_target: ObjectId,
    dredger: ObjectId,
    base_creatures: [ObjectId; 3],
}

fn fixture(seed: u64) -> Fixture {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let third = PlayerId(2);
    let mut game = Game::new(definitions(), 3).expect("three-player fixture initializes");
    game.set_shuffle_seed(seed);

    // Enough public library cards support every regular draw in the long trace.
    for player in [first, second, third] {
        for _ in 0..48 {
            game.add_card(player, LIBRARY_FILLER, Zone::Library)
                .expect("known filler enters library");
        }
    }
    let transmute_target = game
        .add_card(first, TRANSMUTE_TARGET, Zone::Library)
        .expect("matching transmute target enters the first library");
    let dredger = game
        .add_card(first, DREDGER, Zone::Graveyard)
        .expect("dredger starts in its owner's graveyard");

    let forest = game
        .add_card(first, FOREST, Zone::Hand)
        .expect("first land enters hand");
    let second_land = game
        .add_card(first, ISLAND, Zone::Hand)
        .expect("second land enters hand");
    let token_maker = game
        .add_card(first, TOKEN_MAKER, Zone::Hand)
        .expect("token maker enters hand");
    let convoke_ritual = game
        .add_card(first, CONVOKE_RITUAL, Zone::Hand)
        .expect("convoke spell enters hand");
    let radiance = game
        .add_card(first, RADIANCE, Zone::Hand)
        .expect("radiance spell enters hand");
    let shrink = game
        .add_card(first, SHRINK, Zone::Hand)
        .expect("shrink spell enters hand");
    let transmuter = game
        .add_card(first, TRANSMUTER, Zone::Hand)
        .expect("transmute card enters hand");
    game.add_card(first, PING, Zone::Hand)
        .expect("first ping enters hand");

    game.add_card(second, MOUNTAIN, Zone::Hand)
        .expect("second player's land enters hand");
    game.add_card(second, PING, Zone::Hand)
        .expect("second player's ping enters hand");
    game.add_card(third, PLAINS, Zone::Hand)
        .expect("third player's land enters hand");
    game.add_card(third, PING, Zone::Hand)
        .expect("third player's ping enters hand");

    let base_creatures = [
        game.put_on_battlefield(first, GREEN_BODY)
            .expect("first convoke body enters"),
        game.put_on_battlefield(first, GREEN_BODY)
            .expect("second convoke body enters"),
        game.put_on_battlefield(first, GREEN_BODY)
            .expect("third convoke body enters"),
    ];
    game.put_on_battlefield(second, RED_BODY)
        .expect("second player creature enters");
    game.put_on_battlefield(second, FRAGILE_BODY)
        .expect("second player fragile creature enters");
    game.put_on_battlefield(third, GREEN_BODY)
        .expect("third player creature enters");
    assert_invariants(&game, seed, 0, "fixture");

    Fixture {
        game,
        forest,
        second_land,
        token_maker,
        convoke_ritual,
        radiance,
        shrink,
        transmuter,
        transmute_target,
        dredger,
        base_creatures,
    }
}

fn trace_context(seed: u64, operation: usize, label: &str, game: &Game) -> String {
    format!(
        "seed=0x{seed:016X} operation={operation} label={label} turn={} step={:?} active={} priority={} stack={} events={:?}",
        game.turn,
        game.step,
        game.active_player.0,
        game.priority.0,
        game.stack.len(),
        game.canonical_event_log(),
    )
}

fn assert_invariants(game: &Game, seed: u64, operation: usize, label: &str) {
    if let Err(error) = game.validate_invariants() {
        panic!(
            "invariant failure: {}: {error}",
            trace_context(seed, operation, label, game)
        );
    }
    for player in &game.players {
        if let Err(error) = game.view_for_player(player.id) {
            panic!(
                "policy view failure for player {}: {}: {error}",
                player.id.0,
                trace_context(seed, operation, label, game),
            );
        }
    }
}

fn submit_accepted(
    game: &mut Game,
    seed: u64,
    operation: usize,
    label: &str,
    player: PlayerId,
    action: PolicyAction,
) {
    let kind = action.kind();
    let action_debug = format!("{action:?}");
    if let Err(error) = game.submit_policy_move(player, POLICY_ID, action) {
        panic!(
            "expected accepted policy action {action_debug}, got {error}: {}",
            trace_context(seed, operation, label, game)
        );
    }
    assert!(
        matches!(
            game.event_log.last(),
            Some(GameEvent::PolicyMoveSubmitted { player: event_player, policy, kind: event_kind })
                if *event_player == player && policy == POLICY_ID && *event_kind == kind
        ),
        "accepted policy action must end in a canonical submission event: {}",
        trace_context(seed, operation, label, game),
    );
    assert_invariants(game, seed, operation, label);
}

fn submit_rejected_atomically(
    game: &mut Game,
    seed: u64,
    operation: usize,
    label: &str,
    player: PlayerId,
    action: PolicyAction,
) {
    let before = ObservableState::capture(game);
    let action_debug = format!("{action:?}");
    let result = game.submit_policy_move(player, POLICY_ID, action);
    assert!(
        result.is_err(),
        "expected rejected policy action {action_debug}: {}",
        trace_context(seed, operation, label, game),
    );
    let after = ObservableState::capture(game);
    assert_eq!(
        after,
        before,
        "rejected policy action must not mutate externally observable state ({action_debug}): {}",
        trace_context(seed, operation, label, game),
    );
    assert_invariants(game, seed, operation, label);
}

fn pass_until_stack_is_empty(game: &mut Game, seed: u64, operation: &mut usize, label: &str) {
    while !game.stack.is_empty() {
        let player = game.priority;
        submit_accepted(
            game,
            seed,
            *operation,
            label,
            player,
            PolicyAction::PassPriority,
        );
        *operation += 1;
    }
}

fn expected_tokens_since(game: &Game, event_start: usize, controller: PlayerId) -> Vec<ObjectId> {
    game.event_log[event_start..]
        .iter()
        .filter_map(|event| match event {
            GameEvent::TokenCreated { player, token } if *player == controller => Some(*token),
            _ => None,
        })
        .collect()
}

fn living_opponent(game: &Game, player: PlayerId) -> PlayerId {
    game.players
        .iter()
        .map(|candidate| candidate.id)
        .find(|candidate| {
            *candidate != player && !game.player(*candidate).expect("seat exists").lost
        })
        .expect("three-player trace always has a living opponent")
}

fn selected_action(game: &Game, player: PlayerId, rng: &mut TraceRng) -> PolicyAction {
    let view = game
        .view_for_player(player)
        .expect("validated state always produces a policy view");
    if view.draw_replacement_pending {
        return PolicyAction::Draw { dredge: None };
    }
    match game.step {
        cardbench_magic_engine::Step::DeclareAttackers if !view.attackers_declared => {
            let legal_attackers: Vec<_> = view
                .own_battlefield
                .iter()
                .filter(|card| card.can_attack)
                .map(|card| card.id)
                .collect();
            let attackers = legal_attackers
                .into_iter()
                .filter(|_| rng.choose(2) == 1)
                .collect();
            PolicyAction::DeclareAttackers { attackers }
        }
        cardbench_magic_engine::Step::DeclareBlockers if !view.blockers_declared => {
            let attacker = view.combat_attackers.first().map(|card| card.id);
            let blocker = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.card_types.contains(&CardType::Creature))
                .map(|card| card.id);
            let assignments = if rng.choose(2) == 1 {
                attacker
                    .zip(blocker)
                    .map(|(attacker, blocker)| vec![CombatBlock { attacker, blocker }])
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            PolicyAction::DeclareBlockers { assignments }
        }
        _ => {
            let mut actions = vec![PolicyAction::PassPriority];
            if player == game.active_player
                && game.step.is_main()
                && game.stack.is_empty()
                && view.lands_played == 0
            {
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
                {
                    actions.push(PolicyAction::PlayLand { card: land.id });
                }
            }
            if let Some((land, color)) = view.own_battlefield.iter().find_map(|card| {
                (!card.tapped)
                    .then(|| {
                        card.mana_colors
                            .iter()
                            .next()
                            .copied()
                            .map(|color| (card.id, color))
                    })
                    .flatten()
            }) {
                actions.push(PolicyAction::ActivateManaAbility { land, color });
            }
            if let Some(card) = view.hand.iter().find(|card| card.definition == Some(PING)) {
                actions.push(PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![Target::Player(living_opponent(game, player))],
                    convoke: vec![],
                }));
            }
            actions.swap_remove(rng.choose(actions.len()))
        }
    }
}

#[allow(clippy::too_many_lines)] // The fixture transcript is intentionally explicit and auditable.
fn run_trace(seed: u64) -> TraceReceipt {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut fixture = fixture(seed);
    let game = &mut fixture.game;
    let mut operation = 1;
    let mut accepted = 0;
    let mut rejected = 0;

    // Wrong priority is a deliberately rejected move at the policy boundary.
    submit_rejected_atomically(
        game,
        seed,
        operation,
        "wrong-priority-pass",
        second,
        PolicyAction::PassPriority,
    );
    operation += 1;
    rejected += 1;

    // A nonland cannot be smuggled through the land-play policy variant.
    submit_rejected_atomically(
        game,
        seed,
        operation,
        "spell-as-land",
        first,
        PolicyAction::PlayLand {
            card: fixture.token_maker,
        },
    );
    operation += 1;
    rejected += 1;

    // A creature is neither a mana source nor a legal color producer.
    submit_rejected_atomically(
        game,
        seed,
        operation,
        "creature-as-mana-source",
        first,
        PolicyAction::ActivateManaAbility {
            land: fixture.base_creatures[0],
            color: Color::Green,
        },
    );
    operation += 1;
    rejected += 1;

    submit_accepted(
        game,
        seed,
        operation,
        "first-land-play",
        first,
        PolicyAction::PlayLand {
            card: fixture.forest,
        },
    );
    operation += 1;
    accepted += 1;

    // The second basic land is otherwise legal, so this specifically exercises
    // the once-per-turn limit and rejection atomicity.
    submit_rejected_atomically(
        game,
        seed,
        operation,
        "second-land-limit",
        first,
        PolicyAction::PlayLand {
            card: fixture.second_land,
        },
    );
    operation += 1;
    rejected += 1;

    // First reject a color the Forest does not produce, then use its legal mana.
    submit_rejected_atomically(
        game,
        seed,
        operation,
        "wrong-land-color",
        first,
        PolicyAction::ActivateManaAbility {
            land: fixture.forest,
            color: Color::Red,
        },
    );
    operation += 1;
    rejected += 1;
    submit_accepted(
        game,
        seed,
        operation,
        "forest-mana",
        first,
        PolicyAction::ActivateManaAbility {
            land: fixture.forest,
            color: Color::Green,
        },
    );
    operation += 1;
    accepted += 1;
    submit_rejected_atomically(
        game,
        seed,
        operation,
        "tapped-land-mana",
        first,
        PolicyAction::ActivateManaAbility {
            land: fixture.forest,
            color: Color::Green,
        },
    );
    operation += 1;
    rejected += 1;

    let token_event_start = game.event_log.len();
    submit_accepted(
        game,
        seed,
        operation,
        "cast-token-maker",
        first,
        PolicyAction::Cast(CastRequest {
            card: fixture.token_maker,
            targets: vec![],
            convoke: vec![],
        }),
    );
    operation += 1;
    accepted += 1;
    pass_until_stack_is_empty(game, seed, &mut operation, "resolve-token-maker");
    accepted += 3;
    let first_tokens = expected_tokens_since(game, token_event_start, first);
    assert_eq!(
        first_tokens.len(),
        2,
        "token-maker must create two tokens: {}",
        trace_context(seed, operation, "token-count", game)
    );

    submit_accepted(
        game,
        seed,
        operation,
        "cast-radiance",
        first,
        PolicyAction::Cast(CastRequest {
            card: fixture.radiance,
            targets: vec![Target::Permanent(fixture.base_creatures[0])],
            convoke: vec![],
        }),
    );
    operation += 1;
    accepted += 1;
    pass_until_stack_is_empty(game, seed, &mut operation, "resolve-radiance");
    accepted += 3;
    assert_eq!(
        game.characteristics(fixture.base_creatures[0])
            .expect("living base creature has characteristics")
            .power,
        Some(3),
        "radiance-like layer must modify every matching green creature: {}",
        trace_context(seed, operation, "radiance-layer", game),
    );

    // Duplicating a convoke creature must fail before the card leaves hand or
    // the permanent is tapped.
    submit_rejected_atomically(
        game,
        seed,
        operation,
        "duplicate-convoke-payment",
        first,
        PolicyAction::Cast(CastRequest {
            card: fixture.convoke_ritual,
            targets: vec![],
            convoke: vec![
                ConvokePayment {
                    creature: fixture.base_creatures[0],
                    contribution: ConvokeContribution::Color(Color::Green),
                },
                ConvokePayment {
                    creature: fixture.base_creatures[0],
                    contribution: ConvokeContribution::Generic,
                },
            ],
        }),
    );
    operation += 1;
    rejected += 1;
    let convoke_event_start = game.event_log.len();
    submit_accepted(
        game,
        seed,
        operation,
        "fully-convoked-spell",
        first,
        PolicyAction::Cast(CastRequest {
            card: fixture.convoke_ritual,
            targets: vec![],
            convoke: vec![
                ConvokePayment {
                    creature: fixture.base_creatures[0],
                    contribution: ConvokeContribution::Color(Color::Green),
                },
                ConvokePayment {
                    creature: fixture.base_creatures[1],
                    contribution: ConvokeContribution::Generic,
                },
                ConvokePayment {
                    creature: fixture.base_creatures[2],
                    contribution: ConvokeContribution::Generic,
                },
            ],
        }),
    );
    operation += 1;
    accepted += 1;
    pass_until_stack_is_empty(game, seed, &mut operation, "resolve-convoke");
    accepted += 3;
    assert_eq!(
        expected_tokens_since(game, convoke_event_start, first).len(),
        1,
        "the fully convoked spell must resolve exactly once: {}",
        trace_context(seed, operation, "convoke-token", game),
    );
    assert!(
        fixture
            .base_creatures
            .iter()
            .all(|card| game.object(*card).expect("convoke body exists").tapped),
        "each selected convoke creature must be tapped: {}",
        trace_context(seed, operation, "convoke-tapping", game),
    );

    // The first token has the radiance +1/+1 modifier.  A -2/-2 target effect
    // causes SBA removal, which in turn exercises the dead-token GameView path.
    submit_accepted(
        game,
        seed,
        operation,
        "kill-token-with-layered-shrink",
        first,
        PolicyAction::Cast(CastRequest {
            card: fixture.shrink,
            targets: vec![Target::Permanent(first_tokens[0])],
            convoke: vec![],
        }),
    );
    operation += 1;
    accepted += 1;
    pass_until_stack_is_empty(game, seed, &mut operation, "resolve-token-shrink");
    accepted += 3;
    assert_eq!(game.zone_of(first_tokens[0]), None);
    assert!(
        game.object(first_tokens[0]).is_err(),
        "SBA-dead tokens must leave no object behind: {}",
        trace_context(seed, operation, "dead-token-removal", game),
    );
    // A view is part of the policy contract; this is the direct regression probe
    // for dead historical combat/token state.
    assert_invariants(game, seed, operation, "dead-token-policy-view");

    // Transmute uses both a hidden-zone selection and deterministic shuffle; the
    // candidate is intentionally a known owner-only object from the first library.
    game.grant_mana(first, Color::Blue, 2)
        .expect("scenario setup grants transmute blue mana");
    game.grant_mana(first, Color::Green, 1)
        .expect("scenario setup grants transmute generic mana");
    assert_invariants(game, seed, operation, "transmute-mana-setup");
    submit_accepted(
        game,
        seed,
        operation,
        "transmute",
        first,
        PolicyAction::Transmute {
            card: fixture.transmuter,
            found: Some(fixture.transmute_target),
        },
    );
    operation += 1;
    accepted += 1;
    assert_eq!(game.zone_of(fixture.transmuter), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(fixture.transmute_target), Some(Zone::Hand));

    // Dredge has no policy action variant; it is still part of the public engine
    // surface and must coexist with the policy-driven state machine.
    game.draw_card(first, Some(fixture.dredger))
        .expect("legal draw replacement dredges");
    operation += 1;
    assert_eq!(game.zone_of(fixture.dredger), Some(Zone::Hand));
    assert_invariants(game, seed, operation, "dredge-draw-replacement");

    // From here, drive a long state machine whose action choice is fully
    // determined by `seed`.  Every seventh iteration asks a non-priority player
    // to pass, exercising a deliberate rejected policy move alongside normal
    // progress.  All ordinary moves use submit_policy_move as a code policy would.
    let mut rng = TraceRng::new(seed);
    for trace_step in 0..180 {
        if game.is_game_over() {
            break;
        }
        assert_invariants(game, seed, operation, "pre-random-action");
        if trace_step % 7 == 0 {
            let wrong_player = PlayerId((game.priority.0 + 1) % game.players.len());
            submit_rejected_atomically(
                game,
                seed,
                operation,
                "seeded-wrong-priority-pass",
                wrong_player,
                PolicyAction::PassPriority,
            );
            operation += 1;
            rejected += 1;
        }
        let player = game.next_policy_player();
        let action = selected_action(game, player, &mut rng);
        submit_accepted(
            game,
            seed,
            operation,
            "seeded-policy-action",
            player,
            action,
        );
        operation += 1;
        accepted += 1;
    }

    assert_eq!(
        accepted,
        199,
        "trace must retain its complete accepted policy transcript: {}",
        trace_context(seed, operation, "accepted-count", game),
    );
    assert_eq!(
        rejected, 33,
        "trace must include every atomic rejection probe"
    );
    assert_invariants(game, seed, operation, "trace-complete");
    TraceReceipt {
        seed,
        accepted,
        rejected,
        events: game.canonical_event_log(),
        final_turn: game.turn,
        final_step: game.step,
        final_life: game.players.iter().map(|player| player.life).collect(),
    }
}

#[test]
fn stateful_policy_campaign_preserves_invariants_across_sixty_four_seeded_traces() {
    let receipts: Vec<_> = (0..64).map(run_trace).collect();
    assert_eq!(receipts.len(), 64);
    assert!(receipts.iter().all(|receipt| receipt.accepted == 199));
    assert!(receipts.iter().all(|receipt| receipt.rejected == 33));
    assert!(
        receipts.iter().all(|receipt| receipt
            .events
            .iter()
            .any(|event| event.contains("ConvokeUsed"))),
        "every trace must execute its deterministic convoke prelude"
    );
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.events.iter().any(|event| event.contains("Dredged"))),
        "every trace must execute its deterministic dredge prelude"
    );
}

#[test]
fn stateful_policy_transcripts_replay_identically_for_the_same_seed() {
    for seed in [0, 1, 0xC0FF_EE12_3456_7890, u64::MAX] {
        let first = run_trace(seed);
        let replay = run_trace(seed);
        assert_eq!(
            first, replay,
            "stateful policy campaign must be reproducible for seed 0x{seed:016X}"
        );
    }
}

#[test]
fn stateful_policy_campaign_uses_all_major_policy_move_kinds() {
    let receipt = run_trace(0xA11C_E5E0_1234_5678);
    let kinds: HashSet<_> = receipt
        .events
        .iter()
        .filter_map(|event| {
            event
                .contains("PolicyMoveSubmitted")
                .then(|| event.split("kind: ").nth(1))
                .flatten()
                .and_then(|tail| tail.split('}').next())
                .map(str::to_owned)
        })
        .collect();
    for expected in [
        "PlayLand",
        "ActivateManaAbility",
        "Cast",
        "Transmute",
        "PassPriority",
        "DeclareAttackers",
        "DeclareBlockers",
    ] {
        assert!(
            kinds.iter().any(|kind| kind.contains(expected)),
            "deterministic policy transcript did not include {expected}: {:#?}",
            receipt.events
        );
    }
}
