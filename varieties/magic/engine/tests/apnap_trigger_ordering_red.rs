//! Red regression for CR 603.3b simultaneous-trigger ordering.
//!
//! Two abilities controlled by the active player trigger together with one
//! controlled by the nonactive player. Their controller, rather than fixture
//! insertion order, must submit the order in which their group is put onto
//! the stack before APNAP continues to the next controller.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionKind, DecisionSelection, Game, GameEvent, ManaCost,
    PlayerId, TriggerCondition, TriggerOrderEntry, TriggeredAbility, TriggeredAbilityBinding, Zone,
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

fn land() -> CardDefinition {
    CardDefinition {
        id: "TST-LAND",
        name: "TST-LAND",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Green]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: true,
        supported_rules: &["synthetic-apnap-order"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn simultaneous_trigger_groups_use_controller_order_then_apnap_stack_order() {
    let definitions = vec![
        source("TST-FIRST"),
        source("TST-SECOND"),
        source("TST-NONACTIVE"),
        land(),
    ];
    let bindings = [
        TriggeredAbilityBinding {
            card_definition: "TST-FIRST",
            ability: TriggeredAbility {
                id: "first-landfall",
                condition: TriggerCondition::LandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "TST-SECOND",
            ability: TriggeredAbility {
                id: "second-landfall",
                condition: TriggerCondition::LandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "TST-NONACTIVE",
            ability: TriggeredAbility {
                id: "nonactive-landfall",
                condition: TriggerCondition::LandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![],
            },
        },
    ];
    let mut game =
        Game::new_with_all_bindings_and_triggers(definitions, 2, [], [], [], [], bindings)
            .expect("synthetic trigger game initializes");
    let first = game
        .add_card(PlayerId(0), "TST-FIRST", Zone::Battlefield)
        .expect("first source enters before start");
    let second = game
        .add_card(PlayerId(0), "TST-SECOND", Zone::Battlefield)
        .expect("second source enters before start");
    let nonactive = game
        .add_card(PlayerId(1), "TST-NONACTIVE", Zone::Battlefield)
        .expect("nonactive source enters before start");
    let played_land = game
        .add_card(PlayerId(0), "TST-LAND", Zone::Hand)
        .expect("land starts in active player's hand");

    game.begin_game().expect("game begins");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("advance to precombat main phase");
    }
    game.clear_event_log();
    game.play_land(PlayerId(0), played_land)
        .expect("land entry observes every controller's simultaneous trigger group");
    let view = game
        .view_for_player(PlayerId(0))
        .expect("active player receives its public view");
    println!(
        "APNAP ordering trace before choice: {:?}",
        game.canonical_event_log()
    );
    let decision = view
        .pending_decision
        .expect("simultaneous same-controller triggers must require an explicit order");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(
        decision.trigger_candidates,
        vec![
            TriggerOrderEntry {
                source: first,
                source_incarnation: 1,
                ability: "first-landfall",
            },
            TriggerOrderEntry {
                source: second,
                source_incarnation: 1,
                ability: "second-landfall",
            },
        ],
        "the decision presents both independently orderable active-player triggers"
    );
    assert!(
        game.stack.is_empty(),
        "no trigger may enter the stack before its controller orders the group"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::TriggerOrder(vec![
            decision.trigger_candidates[1],
            decision.trigger_candidates[0],
        ]),
    )
    .expect("controller reverses its own simultaneous triggers");
    println!(
        "APNAP ordering trace after choice: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.stack
            .iter()
            .map(|object| (object.card, object.ability_id))
            .collect::<Vec<_>>(),
        vec![
            (second, Some("second-landfall")),
            (first, Some("first-landfall")),
            (nonactive, Some("nonactive-landfall")),
        ],
        "active player's selected order is lower on the stack than the nonactive player's group"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityOrderChosen { controller, order }
            if *controller == PlayerId(0)
                && order == &vec![decision.trigger_candidates[1], decision.trigger_candidates[0]]
    )));
    game.validate_invariants()
        .expect("a trigger-order boundary must remain state-machine valid");
}
