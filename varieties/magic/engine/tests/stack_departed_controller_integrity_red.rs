//! Red probe: a departed player's spell cannot remain on the stack.

#[path = "support/stack_fixture.rs"]
mod stack_fixture;

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, ManaCost, PlayerId, RulesError, StackObject,
    StackObjectId, Target, Zone,
};

const INSTANT: &str = "STACK-DEPARTED-CONTROLLER-INSTANT";
const WARMUP: &str = "STACK-DEPARTED-CONTROLLER-WARMUP";

fn game() -> Game {
    Game::new(
        vec![
            CardDefinition {
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
            },
            CardDefinition {
                id: WARMUP,
                name: WARMUP,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: BTreeSet::new(),
                mana_colors: BTreeSet::new(),
                card_types: BTreeSet::from([CardType::Instant]),
                is_basic_land: false,
                supported_rules: &["stack-identity-warmup"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        ],
        3,
    )
    .expect("fixture game initializes")
}

#[test]
fn invariant_audit_rejects_a_lost_players_spell_left_on_the_stack() {
    let departed = PlayerId(1);
    let mut game = game();
    stack_fixture::advance_stack_identity(&mut game, WARMUP);
    let spell = game
        .add_card(departed, INSTANT, Zone::Hand)
        .expect("instant enters the future departed player's hand");
    game.players[departed.0].hand.clear();
    game.stack.push(StackObject {
        id: StackObjectId(1),
        card: spell,
        source_incarnation: 1,
        source_colors: BTreeSet::new(),
        controller: departed,
        ability_id: None,
        ability_definition: None,
        targets: Vec::<Target>::new(),
        target_incarnations: vec![],
        effects: vec![Effect::DealDamageController { amount: 1 }],
        chosen_x: None,
        chosen_color: None,
        chosen_modal_mode: None,
        mana_spent: None,
        convoke_symbols: 0,
        generic_cost_reduction: 0,
    });
    game.players[departed.0].lost = true;

    let audit = game.validate_invariants();
    eprintln!(
        "departed-stack-controller audit result: {audit:?}; lost={:?}; stack: {:?}; events: {:?}",
        departed,
        game.stack,
        game.canonical_event_log()
    );
    assert!(matches!(
        audit,
        Err(RulesError::IllegalAction(
            "a departed player controls a stack object"
        ))
    ));
}
