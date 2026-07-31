//! CR 800.4i regression: an active player's departure must not skip pending stack work.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const ACTIVE_SPELL: &str = "ACTIVE-PLAYER-STACK-BASE";
const SURVIVOR_RESPONSE: &str = "SURVIVOR-STACK-RESPONSE";
const ACTIVE_PLAYER_KILLER: &str = "ACTIVE-PLAYER-STACK-KILLER";

fn instant(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["stack-resolution-transaction-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn active_player_departure_does_not_start_a_new_turn_before_remaining_stack_objects_resolve() {
    let active = PlayerId(0);
    let first_survivor = PlayerId(1);
    let second_survivor = PlayerId(2);
    let mut game = Game::new(
        [
            instant(ACTIVE_SPELL, vec![Effect::GainLifeController { amount: 1 }]),
            instant(
                SURVIVOR_RESPONSE,
                vec![Effect::GainLifeController { amount: 1 }],
            ),
            instant(
                ACTIVE_PLAYER_KILLER,
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
    )
    .expect("three-player fixture initializes");
    let base = game
        .add_card(active, ACTIVE_SPELL, Zone::Hand)
        .expect("active player's base spell enters hand");
    let response = game
        .add_card(first_survivor, SURVIVOR_RESPONSE, Zone::Hand)
        .expect("surviving responder's spell enters hand");
    let killer = game
        .add_card(second_survivor, ACTIVE_PLAYER_KILLER, Zone::Hand)
        .expect("lethal response enters hand");

    game.cast_spell(
        active,
        CastRequest {
            card: base,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("active player casts the base spell");
    game.pass_priority(active).expect("active player passes");
    game.cast_spell(
        first_survivor,
        CastRequest {
            card: response,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("first survivor responds");
    game.pass_priority(first_survivor)
        .expect("first survivor passes");
    game.cast_spell(
        second_survivor,
        CastRequest {
            card: killer,
            targets: vec![Target::Player(active)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("second survivor casts the lethal response");
    game.pass_priority(second_survivor)
        .expect("second survivor passes");
    game.pass_priority(active).expect("active player passes");
    game.pass_priority(first_survivor)
        .expect("first survivor's pass resolves only the lethal response");

    assert!(
        game.players[active.0].lost,
        "the active player must have left"
    );
    assert_eq!(
        game.stack.len(),
        1,
        "the surviving response remains on the stack"
    );
    assert_eq!(
        game.stack[0].card, response,
        "LIFO leaves the lower response pending"
    );
    assert_eq!(
        game.turn,
        1,
        "CR 800.4i keeps the departed active player's turn running until the stack/step completes; events: {:?}",
        game.canonical_event_log()
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| { matches!(event, GameEvent::StepBegan { turn: 2, .. }) }),
        "a new turn began before the remaining stack object resolved; events: {:?}",
        game.canonical_event_log()
    );
}
