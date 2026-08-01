//! Red probe: a departed player's spell cannot remain on the stack.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, ManaCost, PlayerId, StackObject, Target, Zone,
};

const INSTANT: &str = "STACK-DEPARTED-CONTROLLER-INSTANT";

fn game() -> Game {
    Game::new(
        vec![CardDefinition {
            id: INSTANT,
            name: INSTANT,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["test-stack-controller"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamageController { amount: 1 }],
        }],
        3,
    )
    .expect("fixture game initializes")
}

#[test]
fn invariant_audit_rejects_a_lost_players_spell_left_on_the_stack() {
    let departed = PlayerId(1);
    let mut game = game();
    let spell = game
        .add_card(departed, INSTANT, Zone::Hand)
        .expect("instant enters the future departed player's hand");
    game.players[departed.0].hand.clear();
    game.stack.push(StackObject {
        card: spell,
        controller: departed,
        ability_id: None,
        targets: Vec::<Target>::new(),
        effects: vec![Effect::DealDamageController { amount: 1 }],
        chosen_x: None,
        mana_spent: None,
    });
    game.players[departed.0].lost = true;

    let audit = game.validate_invariants();
    eprintln!(
        "departed-stack-controller audit result: {audit:?}; lost={:?}; stack: {:?}; events: {:?}",
        departed,
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        audit.is_err(),
        "a player-loss transition must not leave that player's spell on the stack"
    );
}
