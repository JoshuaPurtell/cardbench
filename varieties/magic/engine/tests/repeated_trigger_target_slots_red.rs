//! Red regression: a generic target decision must support repeated legal
//! selections when an ability has repeated non-distinct target slots.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionSelection, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
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
        supported_rules: &["synthetic-repeated-trigger-target-slots"],
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
        game.pass_priority(player).expect("advance to declare attackers");
    }
}

#[test]
fn repeated_non_distinct_trigger_target_slots_accept_the_same_target_twice() {
    let binding = TriggeredAbilityBinding {
        card_definition: "TST-REPEATED-TARGET-TRIGGER",
        ability: TriggeredAbility {
            id: "two-opponent-creature-targets",
            condition: TriggerCondition::Attacks,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![
                TargetRequirement::OpponentCreature,
                TargetRequirement::OpponentCreature,
            ],
            effects: vec![
                Effect::ReturnOpponentCreatureToHand,
                Effect::ReturnOpponentCreatureToHand,
            ],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            creature("TST-REPEATED-TARGET-TRIGGER"),
            creature("TST-ONLY-OPPONENT-CREATURE"),
        ],
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("synthetic game builds");
    let source = game
        .add_card(PlayerId(0), "TST-REPEATED-TARGET-TRIGGER", Zone::Battlefield)
        .expect("source enters");
    let target = game
        .add_card(PlayerId(1), "TST-ONLY-OPPONENT-CREATURE", Zone::Battlefield)
        .expect("only target enters");
    game.set_entered_turn_for_setup(source, 0)
        .expect("source predates turn");
    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[source])
        .expect("source attacks");

    println!(
        "repeated-target trace before decision: {:?}",
        game.canonical_event_log()
    );
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("repeated target slots need one generic decision");
    assert_eq!((decision.min_selections, decision.max_selections), (2, 2));
    assert_eq!(decision.target_candidates, vec![Target::Permanent(target)]);
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Targets(vec![Target::Permanent(target), Target::Permanent(target)]),
    )
    .expect("the same legal non-distinct target may fill both target slots");
    assert_eq!(
        game.stack.last().expect("trigger stacked").targets,
        vec![Target::Permanent(target), Target::Permanent(target)]
    );
    game.validate_invariants()
        .expect("repeated target-slot decision remains state-machine valid");
}
