//! Red regression for target preservation on state-based-action dies triggers.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Duration, Effect, Game, ManaCost, PlayerId,
    PolicyAction, Target, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

fn definitions() -> Vec<CardDefinition> {
    vec![CardDefinition {
        id: "DIES-TARGET-SOURCE",
        name: "Dies Target Source",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["test-only"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }]
}

#[test]
fn dies_trigger_preserves_its_declared_player_target_on_the_stack() {
    let binding = TriggeredAbilityBinding {
        card_definition: "DIES-TARGET-SOURCE",
        ability: TriggeredAbility {
            id: "dies-target-player",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Player],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions(),
        2,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        [binding],
    )
    .expect("test game builds");
    let source = game
        .add_card(PlayerId(0), "DIES-TARGET-SOURCE", Zone::Battlefield)
        .expect("source enters battlefield");
    game.begin_game().expect("game begins");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player).expect("advance to main phase");
    }
    game.add_continuous_effect(
        source,
        source,
        ContinuousChange::ModifyPowerToughness {
            power: -1,
            toughness: -1,
        },
        Duration::EndOfTurn(1),
    )
    .expect("SBA destroys the source");

    let choice = game
        .view_for_player(PlayerId(0))
        .expect("source controller view")
        .triggered_ability_target_choice
        .expect("dies trigger waits for a controller target choice");
    assert_eq!(choice.source, source);
    assert_eq!(choice.ability, "dies-target-player");
    assert_eq!(
        choice.target_options,
        [vec![
            Target::Player(PlayerId(0)),
            Target::Player(PlayerId(1))
        ]]
    );
    game.submit_policy_move(
        PlayerId(0),
        "test.dies-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source,
            ability: "dies-target-player",
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("controller chooses the dies trigger target");

    println!("dies-trigger red trace: {:?}", game.canonical_event_log());
    assert_eq!(
        game.stack.last().map(|object| object.targets.clone()),
        Some(vec![Target::Player(PlayerId(1))]),
        "the trigger must retain its selected target until stack resolution"
    );
}
