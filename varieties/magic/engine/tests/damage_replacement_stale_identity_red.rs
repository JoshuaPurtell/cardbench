//! Red regression: legacy damage-replacement actions must not replay across
//! two otherwise-identical prospective packets from one resolving spell.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DamageReplacementChoice,
    DamageReplacementEffect, DamageReplacementEffectBinding, Effect, Game, ManaCost, PlayerId,
    PolicyAction, Target, TargetRequirement, Zone,
};

const DOUBLE_BOLT: &str = "TST-STALE-DAMAGE-REPLACEMENT-DOUBLE-BOLT";
const HALVER: &str = "TST-STALE-DAMAGE-REPLACEMENT-HALVER";
const TARGET: &str = "TST-STALE-DAMAGE-REPLACEMENT-TARGET";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["damage-replacement-stale-identity-red"],
        power: (id == TARGET).then_some(10),
        toughness: (id == TARGET).then_some(10),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

#[test]
fn stale_damage_replacement_action_cannot_answer_the_next_identical_packet() {
    let caster = PlayerId(0);
    let mut game = Game::new(
        [
            definition(
                DOUBLE_BOLT,
                BTreeSet::from([CardType::Instant]),
                vec![
                    Effect::DealDamage {
                        amount: 2,
                        target: TargetRequirement::Creature,
                    },
                    Effect::DealDamage {
                        amount: 2,
                        target: TargetRequirement::Creature,
                    },
                ],
            ),
            definition(HALVER, BTreeSet::from([CardType::Enchantment]), vec![]),
            definition(TARGET, BTreeSet::from([CardType::Creature]), vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    game.register_damage_replacement_effect_bindings([DamageReplacementEffectBinding {
        source_definition: HALVER,
        effect: DamageReplacementEffect::HalveDamage,
    }])
    .expect("halving binding registers before game start");
    game.put_on_battlefield(caster, HALVER)
        .expect("first halver enters");
    game.put_on_battlefield(caster, HALVER)
        .expect("second halver enters");
    let target = game
        .put_on_battlefield(caster, TARGET)
        .expect("target enters");
    let bolt = game
        .add_card(caster, DOUBLE_BOLT, Zone::Hand)
        .expect("double bolt enters hand");
    game.begin_game().expect("game begins");
    game.cast_spell(
        caster,
        CastRequest {
            card: bolt,
            targets: vec![Target::Permanent(target), Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("double bolt casts");
    pass_pair(&mut game);

    let first = game
        .view_for_player(caster)
        .expect("caster view")
        .damage_replacement_choice
        .expect("first packet requires a choice");
    let replacement = first
        .replacements
        .iter()
        .copied()
        .find(|candidate| matches!(candidate, DamageReplacementChoice::HalveDamage { .. }))
        .expect("halving candidate exists");
    let stale_action = PolicyAction::ChooseDamageReplacement {
        source: first.source,
        source_incarnation: first.source_incarnation,
        target: first.target,
        replacement,
    };
    game.submit_policy_move(caster, "stale-damage-replacement-red", stale_action.clone())
        .expect("first replacement choice is accepted");

    let second = game
        .view_for_player(caster)
        .expect("caster view")
        .damage_replacement_choice
        .expect("second identical packet requires a new choice");
    assert_eq!(first.source, second.source);
    assert_eq!(first.source_incarnation, second.source_incarnation);
    assert_eq!(first.target, second.target);
    assert_eq!(first.amount, second.amount);
    assert!(
        game.view_for_player(caster)
            .expect("caster view")
            .pending_decision
            .is_some(),
        "the second prompt remains a live generic decision"
    );

    let stale_result = game.submit_policy_move(caster, "stale-damage-replacement-red", stale_action);
    eprintln!(
        "stale damage-replacement trace: first={first:?}; second={second:?}; \\
         stale_result={stale_result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        stale_result.is_err(),
        "a legacy replacement action from the first packet must not answer the second packet"
    );
    game.validate_invariants()
        .expect("rejected stale replacement must preserve a valid state");
}
