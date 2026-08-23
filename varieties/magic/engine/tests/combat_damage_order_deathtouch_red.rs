//! Red regression for combat damage order and deathtouch lethal assignment.
//!
//! CR 509.2 makes the attacking player order each multi-block group before
//! either player receives priority.  That order is not the defender's block
//! declaration order.  CR 702.2 makes one positive point from a deathtouch
//! attacker lethal for damage assignment and state-based actions.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DecisionKind, DecisionSelection, Game, GameEvent,
    Keyword, ManaCost, PlayerId, Step, Zone,
};

const LAND: &str = "TST-LAND";
const TRAMPLER: &str = "TST-TRAMPLER";
const DEATHTOUCH_TRAMPLER: &str = "TST-DEATHTOUCH-TRAMPLER";
const SMALL_BLOCKER: &str = "TST-SMALL-BLOCKER";
const LARGE_BLOCKER: &str = "TST-LARGE-BLOCKER";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn creature(
    id: &'static str,
    power: i16,
    toughness: i16,
    keywords: Vec<Keyword>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: colors([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["combat"],
        power: Some(power),
        toughness: Some(toughness),
        keywords,
        effects: vec![],
    }
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
        creature(TRAMPLER, 7, 7, vec![Keyword::Trample]),
        creature(
            DEATHTOUCH_TRAMPLER,
            7,
            7,
            vec![Keyword::Trample, Keyword::Deathtouch],
        ),
        creature(SMALL_BLOCKER, 0, 2, vec![]),
        creature(LARGE_BLOCKER, 0, 4, vec![]),
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

fn declare_two_blockers(
    game: &mut Game,
    attacker: cardbench_magic_engine::ObjectId,
    first: cardbench_magic_engine::ObjectId,
    second: cardbench_magic_engine::ObjectId,
) {
    advance_to_declare_attackers(game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker attacks");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player).expect("advance to blockers");
    }
    game.declare_blockers(
        PlayerId(1),
        &[
            CombatBlock {
                attacker,
                blocker: first,
            },
            CombatBlock {
                attacker,
                blocker: second,
            },
        ],
    )
    .expect("multi-block declaration is legal");
}

fn submit_reverse_damage_order(game: &mut Game) {
    let view = game
        .view_for_player(PlayerId(0))
        .expect("attacker view exposes public combat decision");
    let decision = view
        .pending_decision
        .expect("attacking player must order blockers before priority");
    assert_eq!(decision.kind, DecisionKind::CombatDamageOrder);
    assert_eq!(decision.min_selections, 2);
    assert_eq!(decision.max_selections, 2);
    assert_eq!(decision.candidates.len(), 2);
    let mut reversed = decision
        .candidates
        .into_iter()
        .map(|card| card.id)
        .collect::<Vec<_>>();
    reversed.reverse();
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(reversed),
    )
    .expect("attacker may submit a complete permutation of the blocker group");
}

fn resolve_combat_damage(game: &mut Game) {
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("advance to combat damage");
    }
}

#[test]
#[allow(clippy::too_many_lines)] // Complete multi-block ordering trace is intentionally end-to-end.
fn every_multi_block_group_is_ordered_before_the_priority_window_opens() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    add_library(&mut game, PlayerId(0));
    add_library(&mut game, PlayerId(1));
    let first_attacker = game
        .put_on_battlefield(PlayerId(0), TRAMPLER)
        .expect("first trampler enters");
    let second_attacker = game
        .put_on_battlefield(PlayerId(0), TRAMPLER)
        .expect("second trampler enters");
    let first_small = game
        .put_on_battlefield(PlayerId(1), SMALL_BLOCKER)
        .expect("first small blocker enters");
    let first_large = game
        .put_on_battlefield(PlayerId(1), LARGE_BLOCKER)
        .expect("first large blocker enters");
    let second_small = game
        .put_on_battlefield(PlayerId(1), SMALL_BLOCKER)
        .expect("second small blocker enters");
    let second_large = game
        .put_on_battlefield(PlayerId(1), LARGE_BLOCKER)
        .expect("second large blocker enters");

    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[first_attacker, second_attacker])
        .expect("both attackers attack");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player).expect("advance to blockers");
    }
    game.declare_blockers(
        PlayerId(1),
        &[
            CombatBlock {
                attacker: first_attacker,
                blocker: first_small,
            },
            CombatBlock {
                attacker: first_attacker,
                blocker: first_large,
            },
            CombatBlock {
                attacker: second_attacker,
                blocker: second_small,
            },
            CombatBlock {
                attacker: second_attacker,
                blocker: second_large,
            },
        ],
    )
    .expect("both multi-block groups are legal");

    let first = game
        .view_for_player(PlayerId(0))
        .expect("attacker view")
        .pending_decision
        .expect("first group must be ordered");
    assert!(first.id.0 > 0);
    game.submit_decision(
        PlayerId(0),
        first.id,
        DecisionSelection::Objects(vec![first_large, first_small]),
    )
    .expect("first group order submits");
    let second = game
        .view_for_player(PlayerId(0))
        .expect("attacker view")
        .pending_decision
        .expect("second group must be ordered before priority");
    assert!(second.id.0 > first.id.0);
    game.submit_decision(
        PlayerId(0),
        second.id,
        DecisionSelection::Objects(vec![second_large, second_small]),
    )
    .expect("second group order submits");

    assert_eq!(game.priority, PlayerId(0));
    let orders = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::CombatDamageOrderChosen {
                player,
                attacker,
                blockers,
            } => Some((*player, *attacker, blockers.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        orders,
        vec![
            (PlayerId(0), first_attacker, vec![first_large, first_small]),
            (
                PlayerId(0),
                second_attacker,
                vec![second_large, second_small],
            ),
        ]
    );
    game.validate_invariants()
        .expect("every multi-block group has complete order provenance");
}

#[test]
fn attacking_player_orders_multi_block_damage_before_priority() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    add_library(&mut game, PlayerId(0));
    add_library(&mut game, PlayerId(1));
    let attacker = game
        .put_on_battlefield(PlayerId(0), TRAMPLER)
        .expect("trampler enters");
    let small = game
        .put_on_battlefield(PlayerId(1), SMALL_BLOCKER)
        .expect("small blocker enters");
    let large = game
        .put_on_battlefield(PlayerId(1), LARGE_BLOCKER)
        .expect("large blocker enters");

    game.begin_game().expect("game begins");
    declare_two_blockers(&mut game, attacker, small, large);
    submit_reverse_damage_order(&mut game);
    let event_start = game.event_log.len();
    resolve_combat_damage(&mut game);

    let events = &game.event_log[event_start..];
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 4 }
            if *source == attacker && *permanent == large
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 }
            if *source == attacker && *permanent == small
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 }
            if *source == attacker
    )));
    game.validate_invariants()
        .expect("ordered multi-block combat keeps invariant provenance coherent");
}

#[test]
fn deathtouch_uses_one_damage_as_lethal_for_assignment_and_sbas() {
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    add_library(&mut game, PlayerId(0));
    add_library(&mut game, PlayerId(1));
    let attacker = game
        .put_on_battlefield(PlayerId(0), DEATHTOUCH_TRAMPLER)
        .expect("deathtouch trampler enters");
    let small = game
        .put_on_battlefield(PlayerId(1), SMALL_BLOCKER)
        .expect("small blocker enters");
    let large = game
        .put_on_battlefield(PlayerId(1), LARGE_BLOCKER)
        .expect("large blocker enters");

    game.begin_game().expect("game begins");
    declare_two_blockers(&mut game, attacker, small, large);
    submit_reverse_damage_order(&mut game);
    let event_start = game.event_log.len();
    resolve_combat_damage(&mut game);

    let events = &game.event_log[event_start..];
    for blocker in [small, large] {
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 }
                if *source == attacker && *permanent == blocker
        )));
        assert_eq!(game.zone_of(blocker), Some(Zone::Graveyard));
    }
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 5 }
            if *source == attacker
    )));
    game.validate_invariants()
        .expect("deathtouch combat and state-based actions remain coherent");
}
