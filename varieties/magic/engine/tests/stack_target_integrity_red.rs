//! Stack target identities must name real game entities.
//!
//! A target may become illegal through a legal transition, but an out-of-range
//! seat ID was never a legal target and must not survive the public-state audit
//! as a plausible rules-counter situation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const BOLT: &str = "STACK-TARGET-INTEGRITY-BOLT";

fn game() -> Game {
    Game::new(
        vec![CardDefinition {
            id: BOLT,
            name: BOLT,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["target-integrity-contract"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
        }],
        2,
    )
    .expect("fixture game initializes")
}

#[test]
fn invariant_audit_rejects_an_out_of_range_stack_player_target() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let spell = game
        .add_card(caster, BOLT, Zone::Hand)
        .expect("spell enters the caster's hand");
    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![Target::Player(opponent)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the legal spell is cast through the normal transition");

    game.stack[0].targets = vec![Target::Player(PlayerId(99))];
    let audit = game.validate_invariants();
    eprintln!(
        "fabricated stack-target audit result: {audit:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        audit.is_err(),
        "a stack target naming an unseated player must be rejected as fabricated state"
    );
}
