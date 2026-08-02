//! Red regression: target-bearing triggered abilities must use the canonical
//! id-bearing decision boundary, rather than a parallel anonymous queue.
//!
//! The fixture intentionally uses synthetic engine definitions.  It exercises
//! core trigger scheduling only; it neither promotes nor depends on a RAV card.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionSelection, Game, GameEvent, ManaCost, PlayerId,
    Target, TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["synthetic-generic-trigger-target-decision"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn advance_to_declare_attackers(game: &mut Game) {
    for player in [
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
    ] {
        game.pass_priority(player)
            .expect("advance to declare attackers");
    }
}

#[test]
fn trigger_target_choice_has_a_typed_decision_id_and_stale_response_guard() {
    let definitions = vec![
        creature("TST-TARGETED-ATTACK-TRIGGER"),
        creature("TST-FIRST-TARGET"),
        creature("TST-SECOND-TARGET"),
    ];
    let binding = TriggeredAbilityBinding {
        card_definition: "TST-TARGETED-ATTACK-TRIGGER",
        ability: TriggeredAbility {
            id: "target-on-attack",
            condition: TriggerCondition::Attacks,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Creature],
            effects: vec![
                cardbench_magic_engine::Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 0,
                    toughness: 0,
                },
            ],
        },
    };
    let mut game =
        Game::new_with_all_bindings_and_triggers(definitions, 2, [], [], [], [], [binding])
            .expect("synthetic target-trigger game builds");
    let source = game
        .add_card(
            PlayerId(0),
            "TST-TARGETED-ATTACK-TRIGGER",
            Zone::Battlefield,
        )
        .expect("source begins on battlefield");
    let first = game
        .add_card(PlayerId(1), "TST-FIRST-TARGET", Zone::Battlefield)
        .expect("first target begins on battlefield");
    let second = game
        .add_card(PlayerId(1), "TST-SECOND-TARGET", Zone::Battlefield)
        .expect("second target begins on battlefield");
    game.set_entered_turn_for_setup(source, 0)
        .expect("source predates the attack turn");
    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);

    game.declare_attackers(PlayerId(0), &[source])
        .expect("source attacks");
    println!(
        "target-trigger trace before generic decision: {:?}",
        game.canonical_event_log()
    );

    let view = game
        .view_for_player(PlayerId(0))
        .expect("controller receives a public view");
    let decision = view
        .pending_decision
        .expect("target-bearing trigger must open a generic decision");
    assert!(
        decision
            .target_candidates
            .contains(&Target::Permanent(first))
            && decision
                .target_candidates
                .contains(&Target::Permanent(second)),
        "the typed decision exposes the legal public target identities"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened { decision: opened, .. } if *opened == decision.id
    )));
    assert_eq!(game.stack.len(), 0, "the trigger waits for its decision");

    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Targets(vec![Target::Permanent(second)]),
    )
    .expect("controller selects the second target through the generic boundary");
    println!(
        "target-trigger trace after generic decision: {:?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionCompleted { decision: completed, .. } if *completed == decision.id
    )));
    assert!(matches!(
        game.stack.last(),
        Some(stack)
            if stack.card == source
                && stack.ability_id == Some("target-on-attack")
                && stack.targets == vec![Target::Permanent(second)]
    ));
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Targets(vec![Target::Permanent(first)]),
        )
        .is_err(),
        "a completed target decision id cannot mutate a later engine state"
    );
    game.validate_invariants()
        .expect("generic trigger-target transition preserves state-machine invariants");
}
