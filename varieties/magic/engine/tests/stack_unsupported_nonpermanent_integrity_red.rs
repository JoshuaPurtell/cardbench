//! Red probe: unsupported nonpermanent cards must never exist as stack objects.

#[path = "support/stack_fixture.rs"]
mod stack_fixture;

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Game, ManaCost, PlayerId, RulesError, StackObject, StackObjectId,
    Target, Zone,
};

const UNSUPPORTED_INSTANT: &str = "STACK-UNSUPPORTED-INSTANT";
const WARMUP: &str = "STACK-UNSUPPORTED-WARMUP";

fn game() -> Game {
    Game::new(
        vec![
            CardDefinition {
                id: UNSUPPORTED_INSTANT,
                name: UNSUPPORTED_INSTANT,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: BTreeSet::new(),
                mana_colors: BTreeSet::new(),
                card_types: BTreeSet::from([CardType::Instant]),
                is_basic_land: false,
                supported_rules: &["unsupported-front-face"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![],
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
                effects: vec![cardbench_magic_engine::Effect::GainLifeController { amount: 1 }],
            },
        ],
        2,
    )
    .expect("fixture game initializes")
}

#[test]
fn invariant_audit_rejects_an_unsupported_instant_fabricated_onto_the_stack() {
    let controller = PlayerId(0);
    let mut game = game();
    stack_fixture::advance_stack_identity(&mut game, WARMUP);
    let spell = game
        .add_card(controller, UNSUPPORTED_INSTANT, Zone::Hand)
        .expect("unsupported instant enters hand for the cast-gate probe");
    game.players[controller.0].hand.clear();
    game.stack.push(StackObject {
        id: StackObjectId(1),
        card: spell,
        source_incarnation: 1,
        source_colors: BTreeSet::new(),
        controller,
        ability_id: None,
        targets: Vec::<Target>::new(),
        target_incarnations: vec![],
        effects: vec![],
        chosen_x: None,
        chosen_color: None,
        chosen_modal_mode: None,
        mana_spent: None,
        convoke_symbols: 0,
        generic_cost_reduction: 0,
    });

    let audit = game.validate_invariants();
    eprintln!(
        "unsupported-stack-spell audit result: {audit:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(matches!(
        audit,
        Err(RulesError::IllegalAction(
            "an unsupported nonpermanent card occupies the stack"
        ))
    ));
}
