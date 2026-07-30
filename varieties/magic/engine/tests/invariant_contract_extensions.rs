//! Broad public-API regression coverage for the engine invariant contract.
//!
//! These tests deliberately compose ordinary game actions instead of constructing
//! private state.  Each successful transition is immediately audited so that a
//! future failure identifies the rule boundary which first corrupted state.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, ContinuousChange, DeckEntry,
    DeckList, Duration, Effect, Game, GameEvent, Keyword, ManaCost, ObjectId, PlayerId,
    PolicyAction, RulesError, Target, TargetRequirement, TokenSpec, Zone,
};

const PLAINS: &str = "TEST-PLAINS";
const ISLAND: &str = "TEST-ISLAND";
const MOUNTAIN: &str = "TEST-MOUNTAIN";
const FOREST: &str = "TEST-FOREST";
const BEAR: &str = "TEST-BEAR";
const WALL: &str = "TEST-WALL";
const SOURCE: &str = "TEST-SOURCE";
const PING: &str = "TEST-PING";
const KILL_CREATURE: &str = "TEST-KILL-CREATURE";
const GROWTH: &str = "TEST-GROWTH";
const TOKEN_SPELL: &str = "TEST-TOKEN-SPELL";
const RADIANCE_GROWTH: &str = "TEST-RADIANCE-GROWTH";
const RADIANCE_TARGET: &str = "TEST-RADIANCE-TARGET";
const RADIANCE_ALLY: &str = "TEST-RADIANCE-ALLY";
const RADIANCE_OFF_COLOR: &str = "TEST-RADIANCE-OFF-COLOR";
const SORCERY: &str = "TEST-SORCERY";
const CONVOKE_SPELL: &str = "TEST-CONVOKE-SPELL";
const DREDGER: &str = "TEST-DREDGER";
const TRANSMUTER: &str = "TEST-TRANSMUTER";
const MANA_VALUE_THREE: &str = "TEST-MANA-VALUE-THREE";
const HIDDEN_CARD: &str = "TEST-HIDDEN-CARD";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn basic_land(id: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
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

fn creature(
    id: &'static str,
    colors_on_card: impl IntoIterator<Item = Color>,
    power: i16,
    toughness: i16,
    keywords: Vec<Keyword>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: colors(colors_on_card),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(power),
        toughness: Some(toughness),
        keywords,
        effects: vec![],
    }
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

#[allow(clippy::too_many_lines)] // Declarative adversarial fixture catalog is intentionally local.
fn definitions() -> Vec<CardDefinition> {
    vec![
        basic_land(PLAINS, Color::White),
        basic_land(ISLAND, Color::Blue),
        basic_land(MOUNTAIN, Color::Red),
        basic_land(FOREST, Color::Green),
        creature(BEAR, [Color::Green], 3, 3, vec![]),
        creature(WALL, [Color::White], 0, 4, vec![Keyword::Defender]),
        creature(SOURCE, [Color::Blue], 2, 2, vec![]),
        instant(
            PING,
            ManaCost::new(0),
            vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
        ),
        instant(
            KILL_CREATURE,
            ManaCost::new(0),
            vec![Effect::DealDamage {
                amount: 5,
                target: TargetRequirement::Creature,
            }],
        ),
        instant(
            GROWTH,
            ManaCost::new(0),
            vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: 2,
                toughness: 2,
            }],
        ),
        instant(
            TOKEN_SPELL,
            ManaCost::new(0),
            vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 2,
            }],
        ),
        instant(
            RADIANCE_GROWTH,
            ManaCost::new(0),
            vec![Effect::RadianceModifyPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            }],
        ),
        creature(RADIANCE_TARGET, [Color::Red], 2, 2, vec![]),
        creature(RADIANCE_ALLY, [Color::Red, Color::White], 3, 3, vec![]),
        creature(RADIANCE_OFF_COLOR, [Color::Green], 4, 4, vec![]),
        CardDefinition {
            id: SORCERY,
            name: SORCERY,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["test-effect"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 1,
            }],
        },
        CardDefinition {
            id: CONVOKE_SPELL,
            name: CONVOKE_SPELL,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["convoke", "test-effect"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 1,
            }],
        },
        creature(DREDGER, [Color::Black], 1, 1, vec![Keyword::Dredge(2)]),
        CardDefinition {
            id: TRANSMUTER,
            name: TRANSMUTER,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["transmute"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![],
        },
        CardDefinition {
            id: MANA_VALUE_THREE,
            name: MANA_VALUE_THREE,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        creature(HIDDEN_CARD, [Color::Black], 4, 4, vec![]),
    ]
}

fn game(player_count: usize) -> Game {
    Game::new(definitions(), player_count).expect("test game initializes")
}

fn assert_invariants(game: &Game) {
    game.validate_invariants()
        .expect("every publicly reachable state must satisfy engine invariants");
}

fn pass_round(game: &mut Game) {
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
            .expect("the living priority holder may pass");
        assert_invariants(game);
    }
}

fn advance_to(game: &mut Game, turn: u32, step: cardbench_magic_engine::Step) {
    for _ in 0..128 {
        if game.turn == turn && game.step == step {
            return;
        }
        pass_round(game);
    }
    panic!("game did not reach requested turn and step");
}

fn cast(game: &mut Game, player: PlayerId, card: ObjectId, target: Option<Target>) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets: target.into_iter().collect(),
            convoke: vec![],
        },
    )
    .expect("test spell is legal to cast");
    assert_invariants(game);
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn definitions_in_library(game: &Game, player: PlayerId) -> Vec<&'static str> {
    game.player(player)
        .expect("player exists")
        .library
        .iter()
        .map(|card| {
            game.object(*card)
                .expect("library card exists")
                .definition
                .expect("library card has a definition")
        })
        .collect()
}

#[test]
fn stack_priority_and_payment_boundaries_preserve_state_and_lifo_order() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = game(2);
    let first_spell = game
        .add_card(first, PING, Zone::Hand)
        .expect("first spell enters hand");
    let second_spell = game
        .add_card(second, PING, Zone::Hand)
        .expect("second spell enters hand");

    cast(&mut game, first, first_spell, Some(Target::Player(second)));
    assert_eq!(
        game.priority, first,
        "the caster retains priority after putting a spell on the stack"
    );
    let before_events = game.event_log.clone();
    assert!(matches!(
        game.add_mana_from_action(second, Color::Red, 1),
        Err(RulesError::Priority { .. })
    ));
    assert_eq!(game.event_log, before_events);
    assert_invariants(&game);

    game.pass_priority(first)
        .expect("the caster passes before the opponent responds");
    cast(&mut game, second, second_spell, Some(Target::Player(first)));
    assert_eq!(game.stack.len(), 2);
    pass_round(&mut game);
    assert_eq!(game.stack.len(), 1);
    assert_eq!(
        game.priority, first,
        "active player gets priority after resolution"
    );
    pass_round(&mut game);
    assert!(game.stack.is_empty());
    assert_eq!(game.player(first).expect("player exists").life, 19);
    assert_eq!(game.player(second).expect("player exists").life, 19);
    assert_eq!(
        game.event_log
            .iter()
            .filter_map(|event| match event {
                GameEvent::SpellResolved { card } => Some(*card),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![second_spell, first_spell]
    );
    assert_invariants(&game);
}

#[test]
fn rejected_mana_and_convoke_actions_are_atomic() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game(2);
    let plains = game
        .put_on_battlefield(player, PLAINS)
        .expect("plains enters battlefield");
    let spell = game
        .add_card(player, CONVOKE_SPELL, Zone::Hand)
        .expect("convoke spell enters hand");
    let creature = game
        .put_on_battlefield(player, BEAR)
        .expect("green creature enters battlefield");
    assert_invariants(&game);

    let before = game.clone();
    assert!(matches!(
        game.activate_mana_ability(player, plains, Color::Red),
        Err(RulesError::IllegalAction(_))
    ));
    assert_eq!(game.players, before.players);
    assert_eq!(game.stack, before.stack);
    assert_eq!(game.event_log, before.event_log);
    assert!(!game.object(plains).expect("land exists").tapped);
    assert_invariants(&game);

    let before = game.clone();
    assert!(matches!(
        game.cast_spell(
            player,
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![cardbench_magic_engine::ConvokePayment {
                    creature,
                    contribution: cardbench_magic_engine::ConvokeContribution::Color(Color::Red),
                }],
            },
        ),
        Err(RulesError::IllegalAction(_))
    ));
    assert_eq!(game.players, before.players);
    assert_eq!(game.stack, before.stack);
    assert_eq!(game.event_log, before.event_log);
    assert!(!game.object(creature).expect("creature exists").tapped);
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert_eq!(game.zone_of(plains), Some(Zone::Battlefield));
    assert_ne!(player, opponent);
    assert_invariants(&game);
}

#[test]
fn sorcery_cannot_be_cast_by_nonactive_player_in_response_to_a_spell() {
    let active_player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game(2);
    let instant = game
        .add_card(active_player, TOKEN_SPELL, Zone::Hand)
        .expect("instant enters the active player's hand");
    let sorcery = game
        .add_card(opponent, SORCERY, Zone::Hand)
        .expect("sorcery enters the opponent's hand");

    cast(&mut game, active_player, instant, None);
    assert_eq!(game.priority, active_player);
    game.pass_priority(active_player)
        .expect("the caster passes to open the opponent's response window");
    assert_eq!(game.priority, opponent);
    let before = game.clone();

    assert!(matches!(
        game.cast_spell(
            opponent,
            CastRequest {
                card: sorcery,
                targets: vec![],
                convoke: vec![],
            },
        ),
        Err(RulesError::IllegalAction(
            "non-instant spells require your main phase with an empty stack"
        ))
    ));
    assert_eq!(game.players, before.players);
    assert_eq!(game.stack, before.stack);
    assert_eq!(game.event_log, before.event_log);
    assert_eq!(game.zone_of(sorcery), Some(Zone::Hand));
    assert_invariants(&game);
}

#[test]
#[allow(clippy::too_many_lines)] // The complete layer, duration, and source-liveness transcript is the contract.
fn continuous_effect_layers_expiry_and_source_liveness_are_observable_and_stable() {
    let player = PlayerId(0);
    let mut game = game(2);
    let source = game
        .put_on_battlefield(player, SOURCE)
        .expect("source enters battlefield");
    let target = game
        .put_on_battlefield(player, BEAR)
        .expect("target enters battlefield");
    let growth = game
        .add_card(player, GROWTH, Zone::Hand)
        .expect("growth enters hand");
    let off_battlefield_target = game
        .add_card(player, BEAR, Zone::Hand)
        .expect("off-battlefield target enters hand");
    let before = game.clone();
    assert!(matches!(
        game.add_continuous_effect(
            source,
            off_battlefield_target,
            ContinuousChange::AddColor(Color::Red),
            Duration::Permanent,
        ),
        Err(RulesError::IllegalAction(
            "a permanent continuous effect requires battlefield source and target"
        ))
    ));
    assert_eq!(game.continuous_effects, before.continuous_effects);
    assert_eq!(game.event_log, before.event_log);
    assert!(matches!(
        game.add_continuous_effect(
            source,
            target,
            ContinuousChange::AddColor(Color::Red),
            Duration::EndOfTurn(game.turn + 1),
        ),
        Err(RulesError::IllegalAction(
            "an end-of-turn effect must expire this turn"
        ))
    ));
    assert_eq!(game.continuous_effects, before.continuous_effects);
    assert_eq!(game.event_log, before.event_log);
    assert_invariants(&game);
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::AddCardType(CardType::Land),
        Duration::Permanent,
    )
    .expect("type-layer effect installs");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::AddColor(Color::Blue),
        Duration::Permanent,
    )
    .expect("color-layer effect installs");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::AddKeyword(Keyword::Defender),
        Duration::Permanent,
    )
    .expect("ability-layer effect installs");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ModifyPowerToughness {
            power: 1,
            toughness: 1,
        },
        Duration::Permanent,
    )
    .expect("power/toughness effect installs");
    assert_invariants(&game);
    let characteristics = game.characteristics(target).expect("target exists");
    assert!(characteristics.card_types.contains(&CardType::Land));
    assert!(characteristics.colors.contains(&Color::Blue));
    assert!(characteristics.keywords.contains(&Keyword::Defender));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(4), Some(4))
    );

    cast(&mut game, player, growth, Some(Target::Permanent(target)));
    pass_round(&mut game);
    assert_eq!(
        game.characteristics(target).expect("target exists").power,
        Some(6),
        "the stack-created end-of-turn effect applies while its source is in the graveyard"
    );
    advance_to(&mut game, 2, cardbench_magic_engine::Step::Upkeep);
    assert_eq!(
        game.characteristics(target).expect("target exists").power,
        Some(4),
        "cleanup removes only the end-of-turn effect"
    );
    assert_invariants(&game);

    game.add_continuous_effect(
        source,
        source,
        ContinuousChange::ModifyPowerToughness {
            power: 0,
            toughness: -2,
        },
        Duration::Permanent,
    )
    .expect("source can receive a lethal continuous modification");
    game.check_state_based_actions()
        .expect("state-based actions remove the zero-toughness source");
    assert_eq!(game.zone_of(source), Some(Zone::Graveyard));
    let reverted = game.characteristics(target).expect("target persists");
    assert!(!reverted.card_types.contains(&CardType::Land));
    assert!(!reverted.colors.contains(&Color::Blue));
    assert!(!reverted.keywords.contains(&Keyword::Defender));
    assert_eq!((reverted.power, reverted.toughness), (Some(3), Some(3)));
    assert_invariants(&game);
}

#[test]
fn radiance_pt_only_selects_shared_colors_without_untapping() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game(2);
    let spell = game
        .add_card(player, RADIANCE_GROWTH, Zone::Hand)
        .expect("radiance spell enters hand");
    let target = game
        .put_on_battlefield(player, RADIANCE_TARGET)
        .expect("red target enters battlefield");
    let ally = game
        .put_on_battlefield(opponent, RADIANCE_ALLY)
        .expect("red-white ally enters battlefield");
    let off_color = game
        .put_on_battlefield(opponent, RADIANCE_OFF_COLOR)
        .expect("green creature enters battlefield");
    for card in [target, ally, off_color] {
        game.set_tapped_for_setup(card, true)
            .expect("battlefield setup can mark the creature tapped");
    }

    cast(&mut game, player, spell, Some(Target::Permanent(target)));
    pass_round(&mut game);

    assert_eq!(
        game.characteristics(target).expect("target exists").power,
        Some(3)
    );
    assert_eq!(
        game.characteristics(ally).expect("ally exists").power,
        Some(4),
        "radiance selection crosses controllers"
    );
    assert_eq!(
        game.characteristics(off_color)
            .expect("off-color creature exists")
            .power,
        Some(4),
        "a creature without a shared color remains outside the radiance set"
    );
    for card in [target, ally, off_color] {
        assert!(
            game.object(card).expect("creature exists").tapped,
            "the power/toughness-only radiance operation must not untap {card:?}"
        );
    }
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::ContinuousEffectCreated { .. }))
            .count(),
        2,
        "one temporary layer-7 effect exists for each shared-color creature"
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::PermanentsUntapped { .. })),
        "the no-untap semantic operation must not emit an untap receipt"
    );
    assert_invariants(&game);
}

#[test]
fn deck_loading_shuffle_and_opening_hand_are_deterministic_and_zone_complete() {
    let player = PlayerId(0);
    let list = deck(&[(PLAINS, 3), (ISLAND, 3), (MOUNTAIN, 3)]);
    let mut first = game(2);
    let mut second = game(2);
    first.set_shuffle_seed(0x00C0_FFEE);
    second.set_shuffle_seed(0x00C0_FFEE);
    first
        .load_deck_into_library(player, &list)
        .expect("first deck loads");
    second
        .load_deck_into_library(player, &list)
        .expect("second deck loads");
    assert_eq!(
        definitions_in_library(&first, player),
        definitions_in_library(&second, player),
        "equal seed and deck produce equal ordered libraries"
    );
    first
        .draw_opening_hand(player, 7)
        .expect("opening hand is drawn");
    second
        .draw_opening_hand(player, 7)
        .expect("opening hand is drawn");
    assert_eq!(
        first.player(player).expect("player exists").hand,
        second.player(player).expect("player exists").hand,
        "determinism includes the top-of-library draw order"
    );
    assert_eq!(
        first.player(player).expect("player exists").library.len(),
        2
    );
    assert!(first.event_log.iter().any(|event| {
        matches!(event, GameEvent::DeckLoaded { player: loaded, cards: 9 } if *loaded == player)
    }));
    assert!(first.event_log.iter().any(|event| {
        matches!(event, GameEvent::LibraryShuffled { player: shuffled, cards: 9 } if *shuffled == player)
    }));
    assert_invariants(&first);
    assert_invariants(&second);
}

#[test]
fn card_views_expose_only_controller_hand_and_legal_transmute_choices() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game(2);
    let transmuter = game
        .add_card(controller, TRANSMUTER, Zone::Hand)
        .expect("transmute card enters hand");
    let legal = game
        .add_card(controller, MANA_VALUE_THREE, Zone::Library)
        .expect("matching card enters own library");
    let _different_value = game
        .add_card(controller, PING, Zone::Library)
        .expect("different mana value card enters own library");
    let hidden_hand = game
        .add_card(opponent, HIDDEN_CARD, Zone::Hand)
        .expect("opponent secret enters hand");
    let hidden_library = game
        .add_card(opponent, HIDDEN_CARD, Zone::Library)
        .expect("opponent secret enters library");
    let public_permanent = game
        .put_on_battlefield(opponent, WALL)
        .expect("opponent permanent is public");
    let view = game.view_for_player(controller).expect("view is available");

    assert_eq!(
        view.hand.iter().map(|card| card.id).collect::<Vec<_>>(),
        vec![transmuter]
    );
    assert_eq!(view.transmute_searches.len(), 1);
    assert_eq!(view.transmute_searches[0].card, transmuter);
    assert_eq!(
        view.transmute_searches[0]
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![legal]
    );
    let view_ids: BTreeSet<_> = view
        .hand
        .iter()
        .chain(
            view.transmute_searches
                .iter()
                .flat_map(|search| search.candidates.iter()),
        )
        .chain(view.own_battlefield.iter())
        .chain(view.opponent_battlefield.iter())
        .map(|card| card.id)
        .collect();
    assert!(!view_ids.contains(&hidden_hand));
    assert!(!view_ids.contains(&hidden_library));
    assert!(view_ids.contains(&public_permanent));
    assert_eq!(
        view.opponent_battlefield
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![public_permanent]
    );
    assert_invariants(&game);
}

#[test]
fn dredge_replaces_only_a_pending_draw_and_preserves_zone_accounting() {
    let player = PlayerId(0);
    let mut game = game(2);
    let dredger = game
        .add_card(player, DREDGER, Zone::Graveyard)
        .expect("dredger enters graveyard");
    let milled_first = game
        .add_card(player, PLAINS, Zone::Library)
        .expect("first card enters library");
    let milled_second = game
        .add_card(player, ISLAND, Zone::Library)
        .expect("second card enters library");
    let before = game.clone();
    assert!(matches!(
        game.dredge(player, dredger),
        Err(RulesError::IllegalAction(
            "dredge may only replace a pending draw"
        ))
    ));
    assert_eq!(game.players, before.players);
    assert_eq!(game.event_log, before.event_log);
    assert_invariants(&game);

    game.draw_card(player, Some(dredger))
        .expect("dredge replaces the draw");
    assert_eq!(game.zone_of(dredger), Some(Zone::Hand));
    assert_eq!(game.zone_of(milled_first), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(milled_second), Some(Zone::Graveyard));
    assert!(
        game.player(player)
            .expect("player exists")
            .library
            .is_empty()
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::Dredged { player: dredging, card, count: 2 } if *dredging == player && *card == dredger)
    }));
    assert_invariants(&game);
}

#[test]
fn transmute_pays_exact_cost_moves_only_legal_cards_and_returns_priority() {
    let player = PlayerId(0);
    let mut game = game(2);
    let transmuter = game
        .add_card(player, TRANSMUTER, Zone::Hand)
        .expect("transmuter enters hand");
    let found = game
        .add_card(player, MANA_VALUE_THREE, Zone::Library)
        .expect("same-value card enters library");
    let rejected = game
        .add_card(player, PING, Zone::Library)
        .expect("wrong-value card enters library");
    game.grant_mana(player, Color::Blue, 2)
        .expect("blue mana setup succeeds");
    game.grant_mana(player, Color::White, 1)
        .expect("generic mana setup succeeds");
    let before_events = game.event_log.clone();
    assert!(matches!(
        game.transmute(player, transmuter, Some(rejected)),
        Err(RulesError::IllegalAction(
            "transmute may find only a card with the discarded card's mana value"
        ))
    ));
    assert_eq!(game.event_log, before_events);
    assert_eq!(game.zone_of(transmuter), Some(Zone::Hand));
    assert_eq!(game.zone_of(rejected), Some(Zone::Library));
    assert_invariants(&game);

    game.transmute(player, transmuter, Some(found))
        .expect("matching transmute resolves");
    assert_eq!(game.zone_of(transmuter), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(found), Some(Zone::Hand));
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .total(),
        0
    );
    assert_eq!(game.priority, player);
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::Transmuted { player: transmuting, discarded, found: selected }
            if *transmuting == player && *discarded == transmuter && *selected == Some(found))
    }));
    assert_invariants(&game);
}

#[test]
fn token_removal_prunes_effects_and_keeps_game_views_total() {
    let player = PlayerId(0);
    let mut game = game(2);
    let token_spell = game
        .add_card(player, TOKEN_SPELL, Zone::Hand)
        .expect("token spell enters hand");
    let stable_target = game
        .put_on_battlefield(player, BEAR)
        .expect("non-token target enters battlefield");
    let source = game
        .put_on_battlefield(player, SOURCE)
        .expect("source enters battlefield");
    cast(&mut game, player, token_spell, None);
    pass_round(&mut game);
    let token = game
        .view_for_player(player)
        .expect("view is available")
        .own_battlefield
        .into_iter()
        .find(|card| card.definition.is_none())
        .expect("token is represented as an object while on the battlefield")
        .id;
    game.add_continuous_effect(
        token,
        stable_target,
        ContinuousChange::ModifyPowerToughness {
            power: 4,
            toughness: 4,
        },
        Duration::Permanent,
    )
    .expect("token can source a permanent effect while it exists");
    game.add_continuous_effect(
        source,
        token,
        ContinuousChange::ModifyPowerToughness {
            power: 0,
            toughness: -1,
        },
        Duration::Permanent,
    )
    .expect("token can receive a lethal continuous effect");
    assert_eq!(
        game.zone_of(token),
        None,
        "a public continuous-effect transition immediately applies lethal SBAs"
    );
    assert!(matches!(game.object(token), Err(RulesError::UnknownCard(card)) if card == token));
    assert_eq!(
        game.characteristics(stable_target)
            .expect("target exists")
            .power,
        Some(3)
    );
    game.check_state_based_actions()
        .expect("the already-stable public transition remains a fixed point");
    let view = game
        .view_for_player(player)
        .expect("dead token does not break views");
    assert!(!view.own_battlefield.iter().any(|card| card.id == token));
    assert_invariants(&game);
}

#[test]
fn combat_declaration_rejections_are_atomic_and_defender_cannot_attack() {
    let attacker_player = PlayerId(0);
    let defender_player = PlayerId(1);
    let mut game = game(2);
    let attacker = game
        .put_on_battlefield(attacker_player, BEAR)
        .expect("attacker enters battlefield");
    let wall = game
        .put_on_battlefield(attacker_player, WALL)
        .expect("wall enters battlefield");
    let blocker = game
        .put_on_battlefield(defender_player, BEAR)
        .expect("blocker enters battlefield");
    game.add_card(attacker_player, PLAINS, Zone::Library)
        .expect("first player has a draw card");
    game.add_card(defender_player, PLAINS, Zone::Library)
        .expect("second player has a draw card");
    advance_to(&mut game, 3, cardbench_magic_engine::Step::DeclareAttackers);
    assert_eq!(game.next_policy_player(), attacker_player);

    let before = game.clone();
    assert!(matches!(
        game.declare_attackers(attacker_player, &[attacker, attacker]),
        Err(RulesError::IllegalAction("an attacker was declared twice"))
    ));
    assert_eq!(game.players, before.players);
    assert_eq!(game.event_log, before.event_log);
    assert_invariants(&game);
    assert!(matches!(
        game.declare_attackers(attacker_player, &[wall]),
        Err(RulesError::IllegalAction("illegal attacker"))
    ));
    assert!(!game.object(wall).expect("wall exists").tapped);
    assert_invariants(&game);

    game.submit_policy_move(
        attacker_player,
        "test.invariant-contract.v1",
        PolicyAction::DeclareAttackers {
            attackers: vec![attacker],
        },
    )
    .expect("legal policy attacker declaration is accepted");
    assert_invariants(&game);
    pass_round(&mut game);
    assert_eq!(game.step, cardbench_magic_engine::Step::DeclareBlockers);
    assert_eq!(game.next_policy_player(), defender_player);
    let before = game.clone();
    assert!(matches!(
        game.declare_blockers(
            defender_player,
            &[CombatBlock {
                attacker,
                blocker: wall,
            }],
        ),
        Err(RulesError::IllegalAction("illegal blocker"))
    ));
    assert_eq!(game.players, before.players);
    assert_eq!(game.event_log, before.event_log);
    assert_invariants(&game);
    game.declare_blockers(defender_player, &[CombatBlock { attacker, blocker }])
        .expect("defending player declares legal blocker");
    assert_eq!(game.priority, attacker_player);
    assert_invariants(&game);
}

#[test]
fn elimination_skips_lost_active_player_when_turn_order_advances() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let third = PlayerId(2);
    let mut game = game(3);
    game.add_card(second, PLAINS, Zone::Library)
        .expect("next active player can draw next turn");
    game.draw_card(first, None)
        .expect("empty-library draw applies loss rule");
    assert!(game.player(first).expect("player exists").lost);
    assert_eq!(game.priority, second);
    assert_eq!(game.next_policy_player(), second);
    assert_invariants(&game);

    advance_to(&mut game, 2, cardbench_magic_engine::Step::Upkeep);
    assert_eq!(game.active_player, second);
    assert_eq!(game.priority, second);
    assert!(!game.player(third).expect("player exists").lost);
    assert_invariants(&game);
}

/// A public-API regression specification for the real Magic rule that an attacker
/// which became blocked stays blocked even if its blocker later leaves play.
#[test]
fn blocked_attacker_stays_blocked_after_its_blocker_leaves_the_battlefield() {
    let attacker_player = PlayerId(0);
    let defender_player = PlayerId(1);
    let mut game = game(2);
    let attacker = game
        .put_on_battlefield(attacker_player, BEAR)
        .expect("attacker enters battlefield");
    let blocker = game
        .put_on_battlefield(defender_player, BEAR)
        .expect("blocker enters battlefield");
    let removal = game
        .add_card(attacker_player, KILL_CREATURE, Zone::Hand)
        .expect("removal spell enters hand");
    game.add_card(attacker_player, PLAINS, Zone::Library)
        .expect("first player has a draw card");
    game.add_card(defender_player, PLAINS, Zone::Library)
        .expect("second player has a draw card");
    advance_to(&mut game, 3, cardbench_magic_engine::Step::DeclareAttackers);
    game.declare_attackers(attacker_player, &[attacker])
        .expect("attacker is legal");
    pass_round(&mut game);
    game.declare_blockers(defender_player, &[CombatBlock { attacker, blocker }])
        .expect("blocker is legal");
    cast(
        &mut game,
        attacker_player,
        removal,
        Some(Target::Permanent(blocker)),
    );
    let first_pass = game.priority;
    game.pass_priority(first_pass)
        .expect("priority passes to the spell controller");
    let second_pass = game.priority;
    game.pass_priority(second_pass)
        .expect("removal resolves without a malformed state transition error");
    assert_eq!(game.zone_of(blocker), Some(Zone::Graveyard));
    let first_pass = game.priority;
    game.pass_priority(first_pass)
        .expect("priority passes toward combat damage");
    let second_pass = game.priority;
    game.pass_priority(second_pass)
        .expect("combat damage begins after both players pass");
    assert_eq!(game.step, cardbench_magic_engine::Step::CombatDamage);
    assert_eq!(
        game.player(defender_player).expect("player exists").life,
        20,
        "a blocked attacker without trample must not deal combat damage to the defending player"
    );
    assert_invariants(&game);
}

/// A malformed deck list is a fail-closed transaction: it neither loads valid
/// preceding entries nor leaves unreachable objects behind.
#[test]
fn rejected_deck_load_is_atomic_and_leaves_no_orphaned_objects() {
    let player = PlayerId(0);
    let mut game = game(2);
    let malformed = deck(&[(PLAINS, 2), ("TEST-UNKNOWN", 1)]);
    let before = game.clone();
    assert!(matches!(
        game.load_deck_into_library(player, &malformed),
        Err(RulesError::UnknownDefinition(
            "deck card missing from catalog"
        ))
    ));
    assert_eq!(game.players, before.players);
    assert_eq!(game.event_log, before.event_log);
    assert_invariants(&game);
}
