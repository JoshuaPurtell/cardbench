//! Red regression: a departed spell target must retain its cast-time card kind.
//!
//! Dynamic legality allows a legal instant/sorcery target to leave the stack
//! before a counterspell resolves.  It does not make a creature card in a
//! graveyard a plausible historical target: that card was never a legal target
//! for this effect at cast time.  The public-state invariant audit must keep
//! those two cases distinct.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, ManaCost, PlayerId, Target, Zone,
};

const LOWER_INSTANT: &str = "STACK-DEPARTED-TARGET-LOWER";
const COUNTER: &str = "STACK-DEPARTED-TARGET-COUNTER";
const CREATURE: &str = "STACK-DEPARTED-TARGET-CREATURE";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let is_creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["departed-spell-target-kind-probe"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

#[test]
fn invariant_rejects_a_counterspell_retargeted_to_a_departed_creature_card() {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                LOWER_INSTANT,
                CardType::Instant,
                vec![Effect::GainLifeController { amount: 1 }],
            ),
            definition(
                COUNTER,
                CardType::Instant,
                vec![Effect::CounterTargetInstantOrSorcerySpell],
            ),
            definition(CREATURE, CardType::Creature, vec![]),
        ],
        2,
    )
    .expect("fixture game initializes");
    let lower = game
        .add_card(caster, LOWER_INSTANT, Zone::Hand)
        .expect("lower instant enters hand");
    let counter = game
        .add_card(responder, COUNTER, Zone::Hand)
        .expect("counterspell enters hand");
    let departed_creature = game
        .add_card(caster, CREATURE, Zone::Graveyard)
        .expect("creature is a departed card");

    game.cast_spell(
        caster,
        CastRequest {
            card: lower,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("lower instant is legally cast");
    game.pass_priority(caster)
        .expect("caster opens a response window");
    game.cast_spell(
        responder,
        CastRequest {
            card: counter,
            targets: vec![Target::Spell(lower)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("counterspell legally targets the lower instant");

    // This is deliberately an external corruption: the spell-shaped target
    // points at an object which has left the stack, so the existing audit
    // treats it as merely dynamically illegal without confirming the fixed
    // instant-or-sorcery target class.
    game.stack[1].targets = vec![Target::Spell(departed_creature)];
    let audit = game.validate_invariants();
    eprintln!(
        "departed non-spell target audit: {audit:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log(),
    );

    assert!(
        audit.is_err(),
        "a departed creature card cannot be a historical instant-or-sorcery target"
    );
}

#[test]
fn invariant_accepts_a_formerly_legal_departed_instant_target() {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                LOWER_INSTANT,
                CardType::Instant,
                vec![Effect::GainLifeController { amount: 1 }],
            ),
            definition(
                COUNTER,
                CardType::Instant,
                vec![Effect::CounterTargetInstantOrSorcerySpell],
            ),
        ],
        2,
    )
    .expect("fixture game initializes");
    let lower = game
        .add_card(caster, LOWER_INSTANT, Zone::Hand)
        .expect("lower instant enters hand");
    let counter = game
        .add_card(responder, COUNTER, Zone::Hand)
        .expect("counterspell enters hand");

    game.cast_spell(
        caster,
        CastRequest {
            card: lower,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("lower instant is legally cast");
    game.pass_priority(caster)
        .expect("caster opens a response window");
    game.cast_spell(
        responder,
        CastRequest {
            card: counter,
            targets: vec![Target::Spell(lower)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("counterspell legally targets the lower instant");

    game.set_fixture_player_life(caster, 0)
        .expect("fixture marks caster at zero life");
    game.check_state_based_actions()
        .expect("owner departure removes the lower spell atomically");

    assert!(game.zone_of(lower).is_none());
    assert!(game.stack.iter().any(|stack| stack.card == counter));
    game.validate_invariants()
        .expect("a legal departed instant target remains a valid historical target");
}
