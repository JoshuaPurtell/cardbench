//! Red regression probe for a defending player leaving after attackers exist.
//!
//! The current substrate represents combat as attacking the next seated
//! opponent.  That defender is fixed when attackers are declared: if that
//! player leaves before blockers, the attackers do not retarget a different
//! surviving seat.  They must leave combat rather than damage that bystander.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, Step, Target, TargetRequirement, Zone,
};

const FILLER: &str = "DEFENDER-DEPARTURE-FILLER";
const ATTACKER: &str = "DEFENDER-DEPARTURE-ATTACKER";
const LETHAL_BURN: &str = "DEFENDER-DEPARTURE-LETHAL-BURN";

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
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(20),
            toughness: Some(20),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: LETHAL_BURN,
            name: LETHAL_BURN,
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
    for _ in 0..256 {
        if game.turn == turn && game.step == step {
            return;
        }
        let view = game
            .view_for_player(game.next_policy_player())
            .expect("current decision view is available");
        if view.draw_replacement_pending {
            let player = game.next_policy_player();
            game.submit_policy_move(
                player,
                "test.defender-departure.v1",
                PolicyAction::Draw { dredge: None },
            )
            .expect("fixture has a card for each ordinary draw");
            continue;
        }
        if game.step == Step::DeclareAttackers && !view.attackers_declared {
            let player = game.next_policy_player();
            game.submit_policy_move(
                player,
                "test.defender-departure.v1",
                PolicyAction::DeclareAttackers { attackers: vec![] },
            )
            .expect("empty attacker declaration is explicit");
            continue;
        }
        if game.step == Step::DeclareBlockers && !view.blockers_declared {
            let player = game.next_policy_player();
            game.submit_policy_move(
                player,
                "test.defender-departure.v1",
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                },
            )
            .expect("empty blocker declaration is explicit");
            continue;
        }
        let player = game.priority;
        game.submit_policy_move(
            player,
            "test.defender-departure.v1",
            PolicyAction::PassPriority,
        )
        .expect("current living priority holder advances the fixture");
    }
    panic!("fixture did not reach turn {turn}, step {step:?}");
}

fn pass_living_players(game: &mut Game) {
    let count = game.players.iter().filter(|player| !player.lost).count();
    for _ in 0..count {
        let player = game.priority;
        game.submit_policy_move(
            player,
            "test.defender-departure.v1",
            PolicyAction::PassPriority,
        )
        .expect("each living player may pass once");
    }
}

#[test]
fn departed_defender_does_not_retarget_declared_attackers_to_next_survivor() {
    let attacker_controller = PlayerId(0);
    let original_defender = PlayerId(1);
    let bystander = PlayerId(2);
    let mut game = Game::new(definitions(), 3).expect("three-player game initializes");
    let attacker = game
        .put_on_battlefield(attacker_controller, ATTACKER)
        .expect("attacker begins on the battlefield");
    let burn = game
        .add_card(attacker_controller, LETHAL_BURN, Zone::Hand)
        .expect("lethal response begins in hand");
    for player in [attacker_controller, original_defender, bystander] {
        for _ in 0..2 {
            game.add_card(player, FILLER, Zone::Library)
                .expect("fixture library prevents a premature draw loss");
        }
    }

    game.begin_game().expect("game starts");
    advance_to(&mut game, 4, Step::DeclareAttackers);
    game.clear_event_log();
    game.submit_policy_move(
        attacker_controller,
        "test.defender-departure.v1",
        PolicyAction::DeclareAttackers {
            attackers: vec![attacker],
        },
    )
    .expect("the turn-four creature may attack the next seated opponent");
    game.submit_policy_move(
        attacker_controller,
        "test.defender-departure.v1",
        PolicyAction::Cast(CastRequest {
            card: burn,
            targets: vec![Target::Player(original_defender)],
            convoke: vec![],
        }),
    )
    .expect("attacker controller has priority after declaring attackers");
    pass_living_players(&mut game);
    assert!(
        game.players[original_defender.0].lost,
        "the attacked seat left the game"
    );
    assert!(
        !game.players[bystander.0].lost,
        "the third seat remains a bystander"
    );
    game.validate_invariants()
        .expect("the loss transition currently accepts this combat state");

    // The next two passes finish the now-defenderless combat.  Current engine
    // behavior instead makes PlayerId(2) declare blockers and then take 20.
    pass_living_players(&mut game);
    assert_eq!(
        game.step,
        Step::DeclareBlockers,
        "current trace reaches a replacement defender"
    );
    game.submit_policy_move(
        bystander,
        "test.defender-departure.v1",
        PolicyAction::DeclareBlockers {
            assignments: vec![],
        },
    )
    .expect("the buggy engine accepts the uninvolved survivor as blocker controller");
    pass_living_players(&mut game);

    assert_eq!(
        game.players[bystander.0].life, 20,
        "a player who was not the defending player when attackers were declared must not take combat damage; events: {:?}",
        game.event_log,
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPlayer { player, .. } if *player == bystander
        )),
        "a defender-departure combat must not redirect its damage receipt to a different player",
    );
}
