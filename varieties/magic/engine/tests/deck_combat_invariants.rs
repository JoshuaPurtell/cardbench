//! Public-API integration coverage for the expansion-neutral game substrate.
//!
//! Each boundary in these tests calls `validate_invariants` so a later scenario
//! failure can be attributed to the first rule transition that corrupts state.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, ContinuousChange, DeckEntry, DeckList, Duration,
    Effect, Game, GameEvent, ManaCost, PlayerId, PolicyAction, PolicyMoveKind, Step, Zone,
};

const PLAINS: &str = "TEST-PLAINS";
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
            id: PLAINS,
            name: "Test Plains",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: colors([Color::White]),
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
            colors: colors([Color::White]),
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
            id: BLOCKER,
            name: "Test Blocker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: Vec::<Effect>::new(),
        },
    ]
}

fn land_deck() -> DeckList {
    DeckList {
        mainboard: vec![DeckEntry {
            card: PLAINS.to_owned(),
            count: 7,
        }],
        sideboard: vec![],
    }
}

fn assert_invariants(game: &Game) {
    game.validate_invariants()
        .expect("public game state must satisfy engine invariants");
}

fn pass_priority_round(game: &mut Game) {
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
    for _ in 0..2 {
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
            .expect("priority holder can pass priority");
        assert_invariants(game);
    }
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    while game.turn != turn || game.step != step {
        pass_priority_round(game);
        assert!(game.turn <= turn, "game advanced beyond desired turn");
    }
}

#[test]
fn deck_opening_hand_policy_mana_and_invariants_use_only_public_api() {
    let player = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    game.set_shuffle_seed(0xD3C5)
        .expect("setup seed is accepted");
    game.load_deck_into_library(player, &land_deck())
        .expect("known deck loads into an empty library");
    assert_invariants(&game);

    game.draw_opening_hand(player, 7)
        .expect("seven-card opening hand draws from loaded deck");
    assert_invariants(&game);
    assert_eq!(game.player(player).expect("player exists").library.len(), 0);
    assert_eq!(game.player(player).expect("player exists").hand.len(), 7);

    let opening_view = game.view_for_player(player).expect("view is available");
    assert_eq!(opening_view.hand.len(), 7);
    let land = opening_view
        .hand
        .iter()
        .find(|card| card.definition == Some(PLAINS))
        .expect("opening hand contains a Plains");
    assert_eq!(land.mana_colors, colors([Color::White]));

    game.submit_policy_move(
        player,
        "test.public-api-policy.v1",
        PolicyAction::PlayLand { card: land.id },
    )
    .expect("a policy may submit a legal land play");
    assert_invariants(&game);
    assert_eq!(game.zone_of(land.id), Some(Zone::Battlefield));

    game.submit_policy_move(
        player,
        "test.public-api-policy.v1",
        PolicyAction::ActivateManaAbility {
            land: land.id,
            color: Color::White,
        },
    )
    .expect("a policy may activate mana listed by the basic land definition");
    assert_invariants(&game);
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .amount(Color::White),
        1
    );
    assert!(game.object(land.id).expect("land exists").tapped);

    let submitted_kinds: Vec<_> = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::PolicyMoveSubmitted { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        submitted_kinds,
        vec![
            PolicyMoveKind::PlayLand,
            PolicyMoveKind::ActivateManaAbility
        ]
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::ManaAbilityActivated {
                player: activated_player,
                land: activated_land,
                color: Color::White,
            } if *activated_player == player && *activated_land == land.id
        )
    }));
}

#[test]
#[allow(clippy::too_many_lines)] // The complete public combat transcript is the test's specification.
fn submitted_combat_moves_deal_damage_run_sbas_and_preserve_invariants() {
    let attacker_controller = PlayerId(0);
    let defending_player = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    // Advancing through the draw steps needs cards in both libraries. Loading is
    // also the public setup path used by complete-match runners.
    game.load_deck_into_library(attacker_controller, &land_deck())
        .expect("first deck loads");
    game.load_deck_into_library(defending_player, &land_deck())
        .expect("second deck loads");
    let blocked_attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("first attacker enters battlefield");
    let unblocked_attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("second attacker enters battlefield");
    let blocker = game
        .put_on_battlefield(defending_player, BLOCKER)
        .expect("blocker enters battlefield");
    assert_invariants(&game);

    // Creatures entered on turn one may attack once player zero receives turn
    // three. `advance_to` deliberately uses only priority actions, exercising
    // the full turn/step sequence rather than mutating internal turn state.
    advance_to(&mut game, 3, Step::DeclareAttackers);
    let attacker_view = game
        .view_for_player(attacker_controller)
        .expect("attacker view is available");
    assert_eq!(attacker_view.decision_player, attacker_controller);
    assert!(
        attacker_view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(ATTACKER))
            .all(|card| card.can_attack)
    );

    game.submit_policy_move(
        attacker_controller,
        "test.combat-attacker.v1",
        PolicyAction::DeclareAttackers {
            attackers: vec![blocked_attacker, unblocked_attacker],
        },
    )
    .expect("policy declares two legal attackers");
    assert_invariants(&game);
    assert_eq!(
        game.view_for_player(attacker_controller)
            .expect("combat view is available")
            .combat_attackers
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![blocked_attacker, unblocked_attacker]
    );

    pass_priority_round(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.submit_policy_move(
        defending_player,
        "test.combat-blocker.v1",
        PolicyAction::DeclareBlockers {
            assignments: vec![CombatBlock {
                attacker: blocked_attacker,
                blocker,
            }],
        },
    )
    .expect("policy declares a legal blocker");
    assert_invariants(&game);

    pass_priority_round(&mut game);
    assert_eq!(game.step, Step::CombatDamage);
    assert_eq!(
        game.player(defending_player)
            .expect("defending player exists")
            .life,
        17,
        "only the unblocked 3-power attacker damages the defending player"
    );
    assert_eq!(game.zone_of(blocker), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(blocked_attacker), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(unblocked_attacker), Some(Zone::Battlefield));
    assert_eq!(
        game.object(blocked_attacker)
            .expect("blocked attacker remains")
            .damage,
        2
    );
    assert_invariants(&game);

    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::DamageDealtToPlayer {
                source,
                player,
                amount: 3,
            } if *source == unblocked_attacker && *player == defending_player
        )
    }));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount: 3,
            } if *source == blocked_attacker && *permanent == blocker
        )
    }));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::StateBasedAction { card, reason }
                if *card == blocker && *reason == "creature has lethal damage"
        )
    }));
    let submitted_kinds: Vec<_> = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::PolicyMoveSubmitted { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        submitted_kinds,
        vec![
            PolicyMoveKind::DeclareAttackers,
            PolicyMoveKind::DeclareBlockers
        ]
    );
}

#[test]
fn nonpositive_power_creatures_deal_no_combat_damage_or_damage_event() {
    let attacker_controller = PlayerId(0);
    let defending_player = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    game.load_deck_into_library(attacker_controller, &land_deck())
        .expect("first deck loads");
    game.load_deck_into_library(defending_player, &land_deck())
        .expect("second deck loads");
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker enters battlefield");
    game.add_continuous_effect(
        attacker,
        attacker,
        ContinuousChange::ModifyPowerToughness {
            power: -4,
            toughness: 0,
        },
        Duration::Permanent,
    )
    .expect("continuous effect reduces the attacker to negative power");

    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.submit_policy_move(
        attacker_controller,
        "test.nonpositive-combat-damage.v1",
        PolicyAction::DeclareAttackers {
            attackers: vec![attacker],
        },
    )
    .expect("negative power does not prevent attacking");
    pass_priority_round(&mut game);
    game.submit_policy_move(
        defending_player,
        "test.nonpositive-combat-damage.v1",
        PolicyAction::DeclareBlockers {
            assignments: vec![],
        },
    )
    .expect("defender declares no blockers");
    pass_priority_round(&mut game);

    assert_eq!(game.step, Step::CombatDamage);
    assert_eq!(
        game.player(defending_player)
            .expect("defending player exists")
            .life,
        20,
        "negative power must not increase the defending player's life"
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPlayer {
                source,
                player,
                ..
            } if *source == attacker && *player == defending_player
        )),
        "zero or negative combat damage is not dealt and has no damage receipt"
    );
    assert_invariants(&game);
}
