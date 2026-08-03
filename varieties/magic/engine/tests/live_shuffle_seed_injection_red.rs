//! Regression for externally mutating the deterministic-shuffle source once a
//! game has crossed its setup boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, RulesError,
    Step, Zone,
};

const TRANSMUTER: &str = "SHUFFLE-SEED-TRANSMUTER";
const MATCH: &str = "SHUFFLE-SEED-MATCH";
const FILLER: &str = "SHUFFLE-SEED-FILLER";
const BLUE_SOURCE: &str = "SHUFFLE-SEED-BLUE-SOURCE";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: TRANSMUTER,
            name: TRANSMUTER,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
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
        CardDefinition {
            id: MATCH,
            name: MATCH,
            set_code: "TST",
            mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: FILLER,
            name: FILLER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: BLUE_SOURCE,
            name: BLUE_SOURCE,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: colors([Color::Blue]),
            card_types: types([CardType::Land]),
            is_basic_land: false,
            supported_rules: &["intrinsic-blue-mana"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn advance_until(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..128 {
        if game.turn == turn && game.step == step {
            return;
        }
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
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("priority passes advance the fixture");
    }
    panic!("fixture did not reach precombat main");
}

fn prepared_game() -> (Game, cardbench_magic_engine::ObjectId) {
    let first = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("fixture initializes");
    game.set_shuffle_seed(0xA11CE)
        .expect("fixture seed is configured during setup");
    game.add_card(first, TRANSMUTER, Zone::Hand)
        .expect("transmuter enters hand during setup");
    let found = game
        .add_card(first, MATCH, Zone::Library)
        .expect("matching card enters library during setup");
    for _ in 0..5 {
        game.add_card(first, FILLER, Zone::Library)
            .expect("filler enters library during setup");
    }
    for _ in 0..3 {
        game.add_card(first, BLUE_SOURCE, Zone::Battlefield)
            .expect("mana source enters the battlefield during setup");
    }
    game.begin_game().expect("prepared game begins");
    advance_until(&mut game, 1, Step::PrecombatMain);
    (game, found)
}

fn resolve_transmute(game: &mut Game, found: cardbench_magic_engine::ObjectId) {
    let first = PlayerId(0);
    let transmuter = game
        .player(first)
        .expect("player exists")
        .hand
        .iter()
        .copied()
        .find(|card| {
            game.card_definition(*card)
                .expect("hand card has definition")
                .id
                == TRANSMUTER
        })
        .expect("transmuter remains in hand");
    let blue_sources: Vec<_> = game
        .player(first)
        .expect("player exists")
        .battlefield
        .iter()
        .copied()
        .filter(|card| {
            game.card_definition(*card)
                .expect("permanent has definition")
                .id
                == BLUE_SOURCE
        })
        .collect();
    assert_eq!(blue_sources.len(), 3, "fixture has three blue sources");
    for source in blue_sources {
        game.activate_mana_ability(first, source, Color::Blue)
            .expect("ordinary intrinsic mana ability pays transmute");
    }
    game.transmute(first, transmuter, Some(found))
        .expect("normal in-game transmute resolves");
}

#[test]
fn live_shuffle_seed_write_cannot_change_a_future_rules_shuffle_without_a_transition() {
    let first = PlayerId(0);
    let (mut baseline, baseline_found) = prepared_game();
    let (mut injected, injected_found) = prepared_game();

    let receipts_before = injected.event_log.len();
    let injection_result = injected.set_shuffle_seed(0xDEAD_BEEF);
    assert!(matches!(
        injection_result,
        Err(RulesError::IllegalAction("shuffle seed is setup-only"))
    ));
    assert_eq!(
        injected.event_log.len(),
        receipts_before,
        "the live external seed write currently has no canonical transition receipt"
    );

    resolve_transmute(&mut baseline, baseline_found);
    resolve_transmute(&mut injected, injected_found);

    assert_eq!(
        injected.player(first).expect("player exists").library,
        baseline.player(first).expect("player exists").library,
        "a live setup seed injection must not control the next rules shuffle"
    );
    assert!(injected.event_log.iter().any(|event| {
        matches!(event, GameEvent::LibraryShuffled { player, cards: 5 } if *player == first)
    }));
    injected
        .validate_invariants()
        .expect("the audit fixture remains shape-valid despite the provenance hole");
}
