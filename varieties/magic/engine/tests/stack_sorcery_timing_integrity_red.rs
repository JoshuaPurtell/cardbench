//! Red probe: stack invariants must reject sorcery-speed spells impossible in
//! the current turn/priority context.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, ManaCost, PlayerId, StackObject, Target, Zone,
};

const SORCERY: &str = "STACK-TIMING-SORCERY";

fn game() -> Game {
    Game::new(
        vec![CardDefinition {
            id: SORCERY,
            name: SORCERY,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["test-stack-timing"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamageController { amount: 1 }],
        }],
        2,
    )
    .expect("fixture game initializes")
}

#[test]
fn invariant_audit_rejects_a_sorcery_fabricated_onto_an_opponents_turn() {
    let active_player = PlayerId(0);
    let nonactive_player = PlayerId(1);
    let mut game = game();
    let spell = game
        .add_card(nonactive_player, SORCERY, Zone::Hand)
        .expect("sorcery enters the nonactive player's hand");
    game.players[nonactive_player.0].hand.clear();
    game.stack.push(StackObject {
        card: spell,
        controller: nonactive_player,
        targets: Vec::<Target>::new(),
        effects: vec![Effect::DealDamageController { amount: 1 }],
    });

    let audit = game.validate_invariants();
    eprintln!(
        "opponent-turn sorcery audit result: {audit:?}; active={active_player:?}; priority={:?}; step={:?}; stack: {:?}; events: {:?}",
        game.priority,
        game.step,
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        audit.is_err(),
        "a sorcery could not have been cast by the nonactive player in this state"
    );
}
