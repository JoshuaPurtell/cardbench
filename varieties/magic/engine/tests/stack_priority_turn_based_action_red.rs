//! Red regression probes for priority versus mandatory combat turn-based actions.
//!
//! These assertions intentionally describe the Rules requirement before the
//! engine is corrected.  They must not be made green in this discovery commit.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, RulesError,
    Step, Target, TargetRequirement, Zone,
};

const PING: &str = "STACK-PRIORITY-RED-PING";
const ATTACKER: &str = "STACK-PRIORITY-RED-ATTACKER";

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: PING,
            name: "Stack Priority Red Ping",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["test-priority"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
        },
        CardDefinition {
            id: ATTACKER,
            name: "Stack Priority Red Attacker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn reach_declare_attackers_before_declaration(game: &mut Game) {
    for _ in 0..4 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("each player may pass through the preceding priority window");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    assert_eq!(game.active_player, PlayerId(0));
    assert_eq!(game.priority, PlayerId(0));
}

#[test]
fn instant_cannot_be_cast_before_mandatory_attacker_declaration() {
    let active = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    let ping = game
        .add_card(active, PING, Zone::Hand)
        .expect("fixture puts the instant into the active player's hand");
    reach_declare_attackers_before_declaration(&mut game);

    let result = game.cast_spell(
        active,
        CastRequest {
            card: ping,
            targets: vec![Target::Player(opponent)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );

    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction(
                "attackers must be declared before priority actions"
            ))
        ),
        "CR 508.1 requires the active player to declare attackers before either player receives priority; result: {result:?}; events: {:?}",
        game.event_log
    );
}

#[test]
fn instant_cannot_be_cast_before_mandatory_blocker_declaration() {
    let active = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("two-player game initializes");
    let ping = game
        .add_card(active, PING, Zone::Hand)
        .expect("fixture puts the instant into the active player's hand");
    let attacker = game
        .put_on_battlefield(active, ATTACKER)
        .expect("fixture puts a legal attacker onto the battlefield");
    // Make the pregame fixture represent a later turn so the creature is not
    // summoning sick when the mandatory declaration is reached.
    game.turn = 2;

    reach_declare_attackers_before_declaration(&mut game);
    game.declare_attackers(active, &[attacker])
        .expect("the active player declares one legal attacker");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("both players pass after attacker declaration");
    }
    assert_eq!(game.step, Step::DeclareBlockers);
    assert_eq!(game.priority, active);

    let result = game.cast_spell(
        active,
        CastRequest {
            card: ping,
            targets: vec![Target::Player(opponent)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );

    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction(
                "blockers must be declared before priority actions"
            ))
        ),
        "CR 509.1 requires the defending player to declare blockers before either player receives priority; result: {result:?}; events: {:?}",
        game.event_log
    );
}
