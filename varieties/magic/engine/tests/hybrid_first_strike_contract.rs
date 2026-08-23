//! Expansion-neutral contracts for two-color hybrid payment and first-strike combat.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, DeckEntry, DeckList, Game,
    GameEvent, HybridManaSymbol, Keyword, ManaCost, PlayerId, Step, Zone,
};

const RECRUIT: &str = "TEST-HYBRID-FIRST-STRIKER";
const BLOCKER: &str = "TEST-NORMAL-BLOCKER";
const DECK_CARD: &str = "TEST-DECK-CARD";

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: RECRUIT,
            name: "Hybrid First Striker",
            set_code: "TST",
            mana_cost: ManaCost::with_hybrid(
                0,
                [],
                [HybridManaSymbol {
                    first: Color::Red,
                    second: Color::White,
                }],
            ),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["hybrid-cost-casting", "first-strike"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::FirstStrike],
            effects: vec![],
        },
        CardDefinition {
            id: BLOCKER,
            name: "Normal Blocker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: DECK_CARD,
            name: "Deck Card",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["deck-filler"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn deck() -> DeckList {
    DeckList {
        mainboard: vec![DeckEntry {
            card: DECK_CARD.to_owned(),
            count: 12,
        }],
        sideboard: vec![],
    }
}

fn pass_round(game: &mut Game) {
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .attackers_declared
    {
        game.declare_attackers(game.next_policy_player(), &[])
            .expect("empty attackers");
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .blockers_declared
    {
        game.declare_blockers(game.next_policy_player(), &[])
            .expect("empty blockers");
    }
    for _ in 0..2 {
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw");
        }
        let player = game.priority;
        game.pass_priority(player).expect("priority pass");
        game.validate_invariants().expect("valid transition");
    }
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..80 {
        if game.turn == turn && game.step == step {
            return;
        }
        pass_round(game);
    }
    panic!("failed to reach turn {turn} step {step:?}");
}

#[test]
fn either_hybrid_color_pays_one_symbol_without_consuming_the_other() {
    for color in [Color::Red, Color::White] {
        let mut game = Game::new(definitions(), 2).expect("game initializes");
        let recruit = game
            .add_card(PlayerId(0), RECRUIT, Zone::Hand)
            .expect("recruit in hand");
        game.grant_mana(PlayerId(0), color, 1)
            .expect("mana setup succeeds");

        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: recruit,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect("either legal hybrid color pays the recruit");
        game.pass_priority(PlayerId(0)).expect("caster passes");
        game.pass_priority(PlayerId(1)).expect("opponent passes");

        assert_eq!(game.zone_of(recruit), Some(Zone::Battlefield));
        assert_eq!(
            game.player(PlayerId(0))
                .expect("player")
                .mana_pool
                .amount(color),
            0
        );
        game.validate_invariants()
            .expect("hybrid transition is valid");
    }
}

#[test]
fn malformed_hybrid_symbol_is_rejected_before_it_can_enter_a_game_catalog() {
    let mut invalid = definitions().into_iter().next().expect("definition");
    invalid.mana_cost = ManaCost::with_hybrid(
        0,
        [],
        [HybridManaSymbol {
            first: Color::Red,
            second: Color::Red,
        }],
    );
    assert!(
        Game::new([invalid], 2).is_err(),
        "a same-color pair is not a hybrid choice"
    );
}

#[test]
fn hybrid_payment_uses_a_feasible_allocation_instead_of_greedy_symbol_order() {
    let mut crossing = definitions().into_iter().next().expect("definition");
    crossing.id = "TEST-CROSSING-HYBRID";
    crossing.mana_cost = ManaCost::with_hybrid(
        0,
        [],
        [
            HybridManaSymbol {
                first: Color::White,
                second: Color::Blue,
            },
            HybridManaSymbol {
                first: Color::White,
                second: Color::Red,
            },
        ],
    );
    let mut game = Game::new([crossing], 2).expect("game initializes");
    let card = game
        .add_card(PlayerId(0), "TEST-CROSSING-HYBRID", Zone::Hand)
        .expect("card in hand");
    game.grant_mana(PlayerId(0), Color::White, 1)
        .expect("white setup");
    game.grant_mana(PlayerId(0), Color::Blue, 1)
        .expect("blue setup");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("white plus blue pays the two overlapping hybrid symbols");
    game.validate_invariants()
        .expect("matching payment leaves a valid stack transition");
}

#[test]
fn missing_hybrid_color_rejects_the_cast_without_a_partial_pool_or_zone_mutation() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    let recruit = game
        .add_card(PlayerId(0), RECRUIT, Zone::Hand)
        .expect("recruit in hand");
    game.grant_mana(PlayerId(0), Color::Blue, 1)
        .expect("unpayable mana setup");
    let events_before = game.canonical_event_log();

    assert!(
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: recruit,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .is_err()
    );
    assert_eq!(game.zone_of(recruit), Some(Zone::Hand));
    assert_eq!(
        game.player(PlayerId(0))
            .expect("player")
            .mana_pool
            .amount(Color::Blue),
        1
    );
    assert_eq!(game.canonical_event_log(), events_before);
    game.validate_invariants()
        .expect("rejected hybrid cast preserves invariants");
}

#[test]
fn first_strike_assigns_before_normal_damage_and_lethal_blocker_cannot_hit_back() {
    let attacker_controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    game.load_deck_into_library(attacker_controller, &deck())
        .expect("attacker deck loads");
    game.load_deck_into_library(defender, &deck())
        .expect("defender deck loads");
    let recruit = game
        .put_on_battlefield(attacker_controller, RECRUIT)
        .expect("recruit enters");
    let blocker = game
        .put_on_battlefield(defender, BLOCKER)
        .expect("blocker enters");
    game.begin_game().expect("game begins");

    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.declare_attackers(attacker_controller, &[recruit])
        .expect("recruit attacks");
    pass_round(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(
        defender,
        &[CombatBlock {
            attacker: recruit,
            blocker,
        }],
    )
    .expect("blocker assigns");
    pass_round(&mut game);

    assert_eq!(game.step, Step::FirstStrikeCombatDamage);
    assert_eq!(game.zone_of(blocker), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(recruit), Some(Zone::Battlefield));
    assert_eq!(game.object(recruit).expect("recruit remains").damage, 0);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 }
            if *source == recruit && *permanent == blocker
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, .. }
            if *source == blocker && *permanent == recruit
    )));

    pass_round(&mut game);
    assert_eq!(game.step, Step::CombatDamage);
    assert_eq!(game.zone_of(recruit), Some(Zone::Battlefield));
    assert_eq!(game.object(recruit).expect("recruit remains").damage, 0);
    game.validate_invariants()
        .expect("first-strike combat remains valid");
}
