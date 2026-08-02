//! Red regression: a departing owner can remove a creature controlled by the
//! active player while it is attacking a different opponent.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ContinuousChange, Duration, Effect, Game,
    GameEvent, ManaCost, PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};

const FILLER: &str = "CONTROLLED-ATTACKER-OWNER-DEPARTURE-FILLER";
const ATTACKER: &str = "CONTROLLED-ATTACKER-OWNER-DEPARTURE-ATTACKER";
const KILLER: &str = "CONTROLLED-ATTACKER-OWNER-DEPARTURE-KILLER";

fn types(card_types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    card_types.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: FILLER,
            name: FILLER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["fixture"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: ATTACKER,
            name: ATTACKER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Green]),
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
            id: KILLER,
            name: KILLER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["damage-player"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 20,
                target: TargetRequirement::Player,
            }],
        },
    ]
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..384 {
        if game.turn == turn && game.step == step {
            return;
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .expect("current policy view is available");
        let action = if view.draw_replacement_pending {
            PolicyAction::Draw { dredge: None }
        } else if game.step == Step::DeclareAttackers && !view.attackers_declared {
            PolicyAction::DeclareAttackers { attackers: vec![] }
        } else if game.step == Step::DeclareBlockers && !view.blockers_declared {
            PolicyAction::DeclareBlockers {
                assignments: vec![],
            }
        } else {
            PolicyAction::PassPriority
        };
        game.submit_policy_move(player, "test.owner-departure.v1", action)
            .expect("fixture advances through one legal public policy action");
    }
    panic!("fixture did not reach turn {turn}, step {step:?}");
}

#[test]
fn owner_departure_removes_a_controlled_attacker_from_live_combat_before_object_removal() {
    let active = PlayerId(0);
    let defender = PlayerId(1);
    let departing_owner = PlayerId(2);
    let killer_controller = PlayerId(3);
    let mut game = Game::new(definitions(), 4).expect("four-player fixture initializes");
    let attacker = game
        .put_on_battlefield(departing_owner, ATTACKER)
        .expect("departing player owns the creature");
    game.add_continuous_effect(
        attacker,
        attacker,
        ContinuousChange::ChangeController(active),
        Duration::Permanent,
    )
    .expect("active player gains control before the game begins");
    let killer = game
        .add_card(killer_controller, KILLER, Zone::Hand)
        .expect("fourth player holds the lethal instant");
    for player in [active, defender, departing_owner, killer_controller] {
        for _ in 0..3 {
            game.add_card(player, FILLER, Zone::Library)
                .expect("fixture library prevents a premature draw loss");
        }
    }

    game.begin_game().expect("game starts");
    advance_to(&mut game, 5, Step::DeclareAttackers);
    game.submit_policy_move(
        active,
        "test.owner-departure.v1",
        PolicyAction::DeclareAttackers {
            attackers: vec![attacker],
        },
    )
    .expect("active player may attack with the stolen creature");

    for expected in [active, defender, departing_owner] {
        assert_eq!(
            game.priority, expected,
            "priority rotates toward the killer"
        );
        game.submit_policy_move(
            expected,
            "test.owner-departure.v1",
            PolicyAction::PassPriority,
        )
        .expect("each earlier player passes priority");
    }
    assert_eq!(game.priority, killer_controller);
    game.submit_policy_move(
        killer_controller,
        "test.owner-departure.v1",
        PolicyAction::Cast(CastRequest {
            card: killer,
            targets: vec![Target::Player(departing_owner)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("fourth player responds by eliminating the attacker's owner");

    for expected in [killer_controller, active, defender] {
        assert_eq!(game.priority, expected, "each survivor passes the response");
        game.submit_policy_move(
            expected,
            "test.owner-departure.v1",
            PolicyAction::PassPriority,
        )
        .expect("the response remains pending until every player passes");
    }
    assert_eq!(game.priority, departing_owner);
    game.submit_policy_move(
        departing_owner,
        "test.owner-departure.v1",
        PolicyAction::PassPriority,
    )
    .expect("owner departure during combat keeps the state machine valid");

    assert!(game.players[departing_owner.0].lost);
    assert!(game.object(attacker).is_err());
    assert!(
        game.view_for_player(active)
            .expect("active player retains a valid policy view")
            .combat_attackers
            .is_empty(),
        "an object that left with its owner cannot remain a live attacker"
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::ObjectLeftGame { object, owner } if *object == attacker && *owner == departing_owner)
    }));
    game.validate_invariants()
        .expect("owner departure leaves no stale live combat membership");
}
