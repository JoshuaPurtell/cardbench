//! Red probe: stack invariants must reject sorcery-speed spells impossible in
//! the current turn/priority context.

#[path = "support/stack_fixture.rs"]
mod stack_fixture;

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, ManaCost, PlayerId, RulesError, StackObject,
    StackObjectId, Target, Zone,
};

const SORCERY: &str = "STACK-TIMING-SORCERY";
const WARMUP: &str = "STACK-TIMING-WARMUP";

fn game() -> Game {
    Game::new(
        vec![
            CardDefinition {
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
        2,
    )
    .expect("fixture game initializes")
}

#[test]
fn invariant_audit_rejects_a_sorcery_fabricated_onto_an_opponents_turn() {
    let active_player = PlayerId(0);
    let nonactive_player = PlayerId(1);
    let mut game = game();
    stack_fixture::advance_stack_identity(&mut game, WARMUP);
    let spell = game
        .add_card(nonactive_player, SORCERY, Zone::Hand)
        .expect("sorcery enters the nonactive player's hand");
    game.players[nonactive_player.0].hand.clear();
    game.stack.push(StackObject {
        id: StackObjectId(1),
        card: spell,
        source_incarnation: 1,
        source_colors: BTreeSet::new(),
        controller: nonactive_player,
        ability_id: None,
        targets: Vec::<Target>::new(),
        target_incarnations: vec![],
        effects: vec![Effect::DealDamageController { amount: 1 }],
        chosen_x: None,
        chosen_color: None,
        mana_spent: None,
        convoke_symbols: 0,
        generic_cost_reduction: 0,
    });

    let audit = game.validate_invariants();
    eprintln!(
        "opponent-turn sorcery audit result: {audit:?}; active={active_player:?}; priority={:?}; step={:?}; stack: {:?}; events: {:?}",
        game.priority,
        game.step,
        game.stack,
        game.canonical_event_log()
    );
    assert!(matches!(
        audit,
        Err(RulesError::IllegalAction(
            "a non-instant stack object has impossible sorcery timing"
        ))
    ));
}
