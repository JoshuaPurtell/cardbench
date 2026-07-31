//! Red probe: stack target identities must also retain a cast-legal target kind.
//!
//! An out-of-range player target is already rejected by the invariant audit, but
//! a `Target::Spell` injected into a player-only spell is equally impossible at
//! cast time. It cannot be explained by a target later becoming illegal: a
//! target never changes from a spell object into a player.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const PLAYER_BOLT: &str = "STACK-TARGET-SHAPE-BOLT";

fn game() -> Game {
    Game::new(
        vec![CardDefinition {
            id: PLAYER_BOLT,
            name: PLAYER_BOLT,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["target-shape-integrity-contract"],
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
fn invariant_audit_rejects_a_spell_target_for_a_player_only_stack_spell() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let spell = game
        .add_card(caster, PLAYER_BOLT, Zone::Hand)
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
    .expect("the player-target spell is cast through the normal transition");

    game.stack[0].targets = vec![Target::Spell(spell)];
    let audit = game.validate_invariants();
    eprintln!(
        "wrong-kind stack-target audit result: {audit:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        audit.is_err(),
        "a player-only spell with a spell target must be rejected as impossible cast history"
    );
}
