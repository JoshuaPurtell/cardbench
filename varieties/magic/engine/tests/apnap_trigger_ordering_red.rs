//! Red regression for CR 603.3b simultaneous-trigger ordering.
//!
//! Two abilities controlled by the active player trigger together at the
//! beginning of upkeep.  Their controller, rather than fixture insertion
//! order, must submit the order in which those abilities are put onto the
//! stack before APNAP continues to the next controller.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, ManaCost, PlayerId, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
};

fn source(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["synthetic-apnap-order"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn simultaneous_same_controller_upkeep_triggers_require_a_policy_order() {
    let definitions = vec![source("TST-FIRST"), source("TST-SECOND")];
    let bindings = [
        TriggeredAbilityBinding {
            card_definition: "TST-FIRST",
            ability: TriggeredAbility {
                id: "first-upkeep",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "TST-SECOND",
            ability: TriggeredAbility {
                id: "second-upkeep",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![],
            },
        },
    ];
    let mut game = Game::new_with_all_bindings_and_triggers(definitions, 2, [], [], [], [], bindings)
        .expect("synthetic trigger game initializes");
    game.add_card(PlayerId(0), "TST-FIRST", Zone::Battlefield)
        .expect("first source enters before start");
    game.add_card(PlayerId(0), "TST-SECOND", Zone::Battlefield)
        .expect("second source enters before start");

    game.begin_game().expect("beginning reaches upkeep");
    let view = game
        .view_for_player(PlayerId(0))
        .expect("active player receives its public view");
    println!("same-controller APNAP red trace: {:?}", game.canonical_event_log());
    assert!(
        view.pending_decision.is_some(),
        "simultaneous same-controller triggers must not silently use fixture order"
    );
    assert!(
        game.stack.is_empty(),
        "no trigger may enter the stack before its controller orders the group"
    );
    game.validate_invariants()
        .expect("a trigger-order boundary must remain state-machine valid");
}
