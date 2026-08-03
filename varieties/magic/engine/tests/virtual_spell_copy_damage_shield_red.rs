//! Red regression: a virtual instant copy may create a target-side prevention
//! shield even though the copy is stack-only.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId,
    Target, Zone,
};

const SHIELD: &str = "TST-VIRTUAL-COPY-SHIELD";
const COPY: &str = "TST-VIRTUAL-COPY-SHIELD-COPY";
const BEAR: &str = "TST-VIRTUAL-COPY-SHIELD-BEAR";

fn definition(id: &'static str, types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    let creature = types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-damage-shield-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn request(card: ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) -> Result<(), cardbench_magic_engine::RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

#[test]
fn virtual_copy_can_create_a_targeted_damage_shield() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                SHIELD,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 3 }],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(BEAR, BTreeSet::from([CardType::Creature]), vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let protected = game
        .put_on_battlefield(copy_controller, BEAR)
        .expect("bear enters before game begins");
    let shield = game
        .add_card(caster, SHIELD, Zone::Hand)
        .expect("shield enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(shield, vec![Target::Permanent(protected)]))
        .expect("shield casts");
    game.pass_priority(caster)
        .expect("caster passes to the copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(shield)]))
        .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction resolves into virtual shield spell");

    let result = resolve_top(&mut game);
    eprintln!(
        "virtual-copy damage-shield red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a copied instant may create a target-side end-of-turn prevention shield"
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageShieldCreated {
                source,
                target: Target::Permanent(target),
                amount: 3,
            } if *target == protected && *source != shield
        )),
        "the virtual copy must retain receipt provenance for its shield"
    );
    game.validate_invariants()
        .expect("a virtual-source prevention shield remains auditable after resolution");
}
