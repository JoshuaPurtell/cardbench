//! Contracts for the model-owned, per-occurrence stack-resolution plan.

use cardbench_magic_engine::{
    Effect, ObjectId, PlayerId, StackEffectResolution, StackObject, StackResolutionPlan, Target,
    TargetRequirement,
};

fn three_target_object(targets: Vec<Target>) -> StackObject {
    StackObject {
        card: ObjectId(7),
        controller: PlayerId(0),
        targets,
        effects: vec![
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
        ],
        mana_spent: None,
    }
}

#[test]
fn plan_preserves_repeated_target_occurrences_and_snapshots_each_legality_once() {
    let target = Target::Permanent(ObjectId(9));
    let object = three_target_object(vec![target, target, target]);
    let mut calls = 0;

    let plan = object
        .resolution_plan(|seen, requirement| {
            assert_eq!(seen, target);
            assert_eq!(requirement, TargetRequirement::Creature);
            calls += 1;
            calls != 2
        })
        .expect("three target slots are present");

    assert_eq!(calls, 3);
    assert_eq!(
        plan,
        StackResolutionPlan::Resolve {
            effects: vec![
                StackEffectResolution::Targeted {
                    target,
                    legal: true,
                },
                StackEffectResolution::Targeted {
                    target,
                    legal: false,
                },
                StackEffectResolution::Targeted {
                    target,
                    legal: true,
                },
            ],
        }
    );
}

#[test]
fn plan_rules_counters_only_when_every_target_occurrence_is_illegal() {
    let object = three_target_object(vec![
        Target::Permanent(ObjectId(1)),
        Target::Permanent(ObjectId(2)),
        Target::Permanent(ObjectId(3)),
    ]);

    let plan = object
        .resolution_plan(|_, _| false)
        .expect("three target slots are present");

    assert_eq!(plan, StackResolutionPlan::CounteredByRules);
}

#[test]
fn plan_rejects_missing_or_extra_target_slots_before_effect_resolution() {
    let object = three_target_object(vec![Target::Permanent(ObjectId(1))]);
    let error = object
        .resolution_plan(|_, _| true)
        .expect_err("one slot cannot satisfy three target occurrences");

    assert_eq!(error.expected, 3);
    assert_eq!(error.actual, 1);
}
