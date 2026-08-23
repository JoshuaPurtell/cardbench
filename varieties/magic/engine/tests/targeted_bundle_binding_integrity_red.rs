//! Red regression: every executable binding must validate shared target bundles.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Effect, Game, ManaCost,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

const SOURCE: &str = "TARGETED-BUNDLE-BINDING-SOURCE";

fn source_definition() -> CardDefinition {
    CardDefinition {
        id: SOURCE,
        name: SOURCE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["targeted-bundle-binding-integrity-probe"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn malformed_bundle() -> Effect {
    Effect::TargetedBundle {
        target: TargetRequirement::Creature,
        effects: vec![],
    }
}

fn valid_bundle() -> Effect {
    Effect::TargetedBundle {
        target: TargetRequirement::Creature,
        effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
            power: 1,
            toughness: 1,
        }],
    }
}

#[test]
fn malformed_shared_target_bundles_are_rejected_for_every_binding_family() {
    let activated = Game::new_with_all_bindings(
        [source_definition()],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: "empty-shared-target-activation",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![malformed_bundle()],
            },
        }],
    );
    assert!(
        activated.is_err(),
        "activated binding accepted malformed shared target bundle: {activated:?}"
    );

    let triggered = Game::new_with_all_bindings_and_triggers(
        [source_definition()],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: SOURCE,
            ability: TriggeredAbility {
                id: "empty-shared-target-trigger",
                condition: TriggerCondition::Attacks,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![TargetRequirement::Creature],
                effects: vec![malformed_bundle()],
            },
        }],
    );
    assert!(
        triggered.is_err(),
        "triggered binding accepted malformed shared target bundle: {triggered:?}"
    );
}

#[test]
fn structurally_valid_shared_target_bundles_remain_available_to_every_binding_family() {
    let activated = Game::new_with_all_bindings(
        [source_definition()],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: "valid-shared-target-activation",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![valid_bundle()],
            },
        }],
    );
    assert!(
        activated.is_ok(),
        "activated binding rejected a structurally valid shared target bundle: {activated:?}"
    );

    let triggered = Game::new_with_all_bindings_and_triggers(
        [source_definition()],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: SOURCE,
            ability: TriggeredAbility {
                id: "valid-shared-target-trigger",
                condition: TriggerCondition::Attacks,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![TargetRequirement::Creature],
                effects: vec![valid_bundle()],
            },
        }],
    );
    assert!(
        triggered.is_ok(),
        "triggered binding rejected a structurally valid shared target bundle: {triggered:?}"
    );
}
