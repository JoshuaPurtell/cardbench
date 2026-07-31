//! Red regression: a stack permanent target must name an actual object.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, ManaCost, ObjectId, PlayerId, Target,
    TargetRequirement, Zone,
};

const BOLT: &str = "STACK-PERMANENT-TARGET-IDENTITY-BOLT";
const CREATURE: &str = "STACK-PERMANENT-TARGET-IDENTITY-CREATURE";

fn definition(id: &'static str, card_types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    let is_creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["stack-permanent-target-identity-probe"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

#[test]
fn invariant_rejects_a_stack_target_rewritten_to_an_unallocated_permanent_id() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                BOLT,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Creature,
                }],
            ),
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
        ],
        2,
    )
    .expect("fixture game initializes");
    let spell = game
        .add_card(caster, BOLT, Zone::Hand)
        .expect("spell enters hand");
    let creature = game
        .add_card(opponent, CREATURE, Zone::Battlefield)
        .expect("creature enters the battlefield");
    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("legal spell is cast through the normal transition");

    game.stack[0].targets = vec![Target::Permanent(ObjectId(99))];
    let audit = game.validate_invariants();
    eprintln!(
        "fabricated permanent-target audit result: {audit:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log(),
    );

    assert!(
        audit.is_err(),
        "a permanent target with no corresponding object could never have been chosen while casting"
    );
}
