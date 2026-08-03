//! Red regression: one cost that sacrifices several creatures is one
//! simultaneous event for last-known-information death triggers.
//!
//! The former sequential implementation removed the first observer before it
//! observed the second sacrifice.  Two "another creature dies" abilities
//! controlled by the payer must therefore create one APNAP ordering decision,
//! rather than silently stacking only the last survivor's trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    DecisionKind, Effect, Game, ManaCost, PlayerId, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

const ALTAR: &str = "TST-SIMULTANEOUS-SACRIFICE-ALTAR";
const OBSERVER_A: &str = "TST-SIMULTANEOUS-SACRIFICE-OBSERVER-A";
const OBSERVER_B: &str = "TST-SIMULTANEOUS-SACRIFICE-OBSERVER-B";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["simultaneous-sacrifice-cost-lki-red"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn altar() -> CardDefinition {
    CardDefinition {
        id: ALTAR,
        name: ALTAR,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["simultaneous-sacrifice-cost-lki-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn sacrificing_two_creatures_as_one_cost_preserves_both_another_dies_observers() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        [altar(), creature(OBSERVER_A), creature(OBSERVER_B)],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: ALTAR,
            ability: ActivatedAbility {
                id: "sacrifice-two",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 2,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
        [
            TriggeredAbilityBinding {
                card_definition: OBSERVER_A,
                ability: TriggeredAbility {
                    id: "another-dies-a",
                    condition: TriggerCondition::AnotherCreatureDies,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 1 }],
                },
            },
            TriggeredAbilityBinding {
                card_definition: OBSERVER_B,
                ability: TriggeredAbility {
                    id: "another-dies-b",
                    condition: TriggerCondition::AnotherCreatureDies,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 1 }],
                },
            },
        ],
    )
    .expect("fixture constructs");
    let altar = game
        .put_on_battlefield(PlayerId(0), ALTAR)
        .expect("altar enters before game start");
    let observer_a = game
        .put_on_battlefield(PlayerId(0), OBSERVER_A)
        .expect("first observer enters before game start");
    let observer_b = game
        .put_on_battlefield(PlayerId(0), OBSERVER_B)
        .expect("second observer enters before game start");
    game.begin_game().expect("game starts");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: altar,
            ability_id: "sacrifice-two",
            sacrifice_sources: vec![observer_a, observer_b],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("the simultaneous sacrifice cost is legal");

    let view = game.view_for_player(PlayerId(0)).expect("controller view");
    eprintln!(
        "simultaneous sacrifice LKI red trace: pending={:?}; stack={:?}; events={:?}",
        view.pending_decision,
        game.stack,
        game.canonical_event_log(),
    );
    let decision = view
        .pending_decision
        .expect("both simultaneous observers require a trigger ordering decision");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(decision.trigger_candidates.len(), 2);
    assert!(
        decision
            .trigger_candidates
            .iter()
            .any(|entry| entry.source == observer_a)
    );
    assert!(
        decision
            .trigger_candidates
            .iter()
            .any(|entry| entry.source == observer_b)
    );
    game.validate_invariants()
        .expect("simultaneous sacrifice trigger boundary remains valid");
}
