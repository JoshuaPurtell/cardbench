//! Red regression: a virtual spell copy retains frozen source facts through
//! automatic damage-amount replacements.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DamageReplacementEffect, DamageReplacementEffectBinding,
    Effect, Game, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const BOLT: &str = "TST-VIRTUAL-COPY-DAMAGE-BOLT";
const COPY: &str = "TST-VIRTUAL-COPY-DAMAGE-COPY";
const HALVER: &str = "TST-VIRTUAL-COPY-DAMAGE-HALVER";

fn definition(id: &'static str, types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-damage-replacement-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn request(card: cardbench_magic_engine::ObjectId, targets: Vec<Target>) -> CastRequest {
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
fn virtual_damage_copy_uses_stack_source_provenance_for_automatic_replacements() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                BOLT,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 3,
                    target: TargetRequirement::Player,
                }],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(HALVER, BTreeSet::from([CardType::Enchantment]), vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    game.register_damage_replacement_effect_bindings([DamageReplacementEffectBinding {
        source_definition: HALVER,
        effect: DamageReplacementEffect::HalveDamage,
    }])
    .expect("halving replacement registers");
    game.put_on_battlefield(caster, HALVER)
        .expect("halver enters before game begins");
    let bolt = game
        .add_card(caster, BOLT, Zone::Hand)
        .expect("bolt enters hand");
    let copy = game
        .add_card(caster, COPY, Zone::Hand)
        .expect("copy enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(bolt, vec![Target::Player(opponent)]))
        .expect("bolt casts");
    game.cast_spell(caster, request(copy, vec![Target::Spell(bolt)]))
        .expect("copy effect casts");
    resolve_top(&mut game).expect("copy effect resolves into a virtual spell");

    let result = resolve_top(&mut game);
    eprintln!(
        "virtual-copy automatic replacement red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a virtual copy must resolve through the live halving replacement"
    );
    assert_eq!(game.player(opponent).expect("opponent exists").life, 19);
    game.validate_invariants()
        .expect("the copied damage replacement has auditable source provenance");
}
