//! Red probe: unsupported nonpermanent cards must never exist as stack objects.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Game, ManaCost, PlayerId, StackObject, Target, Zone,
};

const UNSUPPORTED_INSTANT: &str = "STACK-UNSUPPORTED-INSTANT";

fn game() -> Game {
    Game::new(
        vec![CardDefinition {
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
        }],
        2,
    )
    .expect("fixture game initializes")
}

#[test]
fn invariant_audit_rejects_an_unsupported_instant_fabricated_onto_the_stack() {
    let controller = PlayerId(0);
    let mut game = game();
    let spell = game
        .add_card(controller, UNSUPPORTED_INSTANT, Zone::Hand)
        .expect("unsupported instant enters hand for the cast-gate probe");
    game.players[controller.0].hand.clear();
    game.stack.push(StackObject {
        card: spell,
        controller,
        targets: Vec::<Target>::new(),
        effects: vec![],
    });

    let audit = game.validate_invariants();
    eprintln!(
        "unsupported-stack-spell audit result: {audit:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        audit.is_err(),
        "a nonpermanent card rejected by cast_spell cannot be treated as a valid stack object"
    );
}
