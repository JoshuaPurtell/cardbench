//! The selected X is authoritative stack provenance, not an inferred payment.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Game, ManaCost, PlayerId, StackObject, Target, Zone,
};

const X_SPELL: &str = "CHOSEN-X-STACK-PROBE";
const CREATURE: &str = "CHOSEN-X-CREATURE-PROBE";

fn definition(id: &'static str, types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::with_colors(0, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["chosen-x-stack-probe"],
        power: (id == CREATURE).then_some(1),
        toughness: (id == CREATURE).then_some(1),
        keywords: vec![],
        effects,
    }
}

#[test]
fn invariant_rejects_a_chosen_x_receipt_that_cannot_pay_printed_cost_plus_x() {
    let mut game = Game::new(
        [
            definition(
                X_SPELL,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DestroyTargetCreatureWithManaValueAtMostChosenX],
            ),
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
        ],
        2,
    )
    .expect("fixture game initializes");
    let spell = game
        .add_card(PlayerId(0), X_SPELL, Zone::Hand)
        .expect("spell setup");
    let target = game
        .add_card(PlayerId(1), CREATURE, Zone::Battlefield)
        .expect("target setup");
    game.players[0].hand.clear();
    game.stack.push(StackObject {
        card: spell,
        source_incarnation: 1,
        source_colors: BTreeSet::new(),
        controller: PlayerId(0),
        ability_id: None,
        targets: vec![Target::Permanent(target)],
        target_incarnations: vec![],
        effects: vec![Effect::DestroyTargetCreatureWithManaValueAtMostChosenX],
        chosen_x: Some(2),
        mana_spent: Some(vec![Color::Black]),
        convoke_symbols: 0,
        generic_cost_reduction: 0,
    });

    let audit = game.validate_invariants();
    eprintln!(
        "chosen-X stack audit: {audit:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        audit.is_err(),
        "a fabricated X must not be inferred from an undersized payment receipt"
    );
}
