//! Red regression: optional-trigger choices need the same monotonic identity
//! boundary as every other policy-submitted decision.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color,
    Effect, Game, ManaCost, PlayerId, PolicyAction, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

const SOURCE: &str = "TST-OPTIONAL-TRIGGER-STALE-CHOICE-SOURCE";
const ACTIVATION: &str = "gain-life";
const TRIGGER: &str = "may-turn-life-into-life";

fn source_definition() -> CardDefinition {
    CardDefinition {
        id: SOURCE,
        name: SOURCE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["optional-trigger-stale-choice-identity-red"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn activate_life_gain(
    game: &mut Game,
    controller: PlayerId,
    source: cardbench_magic_engine::ObjectId,
) {
    game.activate_ability(
        controller,
        AbilityActivation {
            source,
            ability_id: ACTIVATION,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("zero-cost life ability activates");
    pass_pair(game);
    pass_pair(game);
}

#[test]
#[allow(clippy::too_many_lines)] // This transcript needs both distinct trigger instances and the stale replay boundary.
fn stale_optional_trigger_response_cannot_answer_a_later_identical_trigger() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [source_definition()],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: ACTIVATION,
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
        [TriggeredAbilityBinding {
            card_definition: SOURCE,
            ability: TriggeredAbility {
                id: TRIGGER,
                condition: TriggerCondition::LifeGained,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 5 }],
            },
        }],
    )
    .expect("fixture builds");
    let source = game
        .put_on_battlefield(controller, SOURCE)
        .expect("source begins on battlefield");
    game.begin_game().expect("game begins");

    activate_life_gain(&mut game, controller, source);
    let first_choice = game
        .view_for_player(controller)
        .expect("controller receives first choice view")
        .optional_triggered_ability_choice
        .expect("first optional trigger waits for controller");
    game.submit_policy_move(
        controller,
        "optional-trigger-stale-choice.first-decline.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: first_choice.decision,
            source,
            ability: TRIGGER,
            pay: false,
            target: None,
        },
    )
    .expect("controller declines first optional trigger");

    activate_life_gain(&mut game, controller, source);
    let second_choice = game
        .view_for_player(controller)
        .expect("controller receives second choice view")
        .optional_triggered_ability_choice
        .expect("second optional trigger waits for controller");
    let stale_result = game.submit_policy_move(
        controller,
        "optional-trigger-stale-choice.replay.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: first_choice.decision,
            source,
            ability: TRIGGER,
            pay: false,
            target: None,
        },
    );

    eprintln!(
        "optional stale-choice red trace: first={first_choice:?}; second={second_choice:?}; result={stale_result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log(),
    );
    assert_ne!(
        first_choice.decision, second_choice.decision,
        "every optional trigger instance needs a fresh monotonic identity"
    );
    assert!(
        stale_result.is_err(),
        "a response captured for the first optional trigger must not be accepted for a later identical trigger"
    );
    game.submit_policy_move(
        controller,
        "optional-trigger-stale-choice.second-decline.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: second_choice.decision,
            source,
            ability: TRIGGER,
            pay: false,
            target: None,
        },
    )
    .expect("the current optional trigger id remains legal");
    game.validate_invariants()
        .expect("fresh optional-trigger identity remains invariant-valid");
}
