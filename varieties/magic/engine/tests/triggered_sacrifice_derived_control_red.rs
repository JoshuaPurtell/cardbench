//! Red regression: trigger-resolution sacrifice choices use derived control,
//! not an object's owner/base-controller field.
//!
//! A "sacrifice a creature you control" trigger may select a creature stolen
//! by a layer-two control effect. The no-priority decision and its resolution
//! must preserve that same derived-control predicate.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, DecisionSelection, Duration, Effect,
    Game, GameEvent, ManaCost, PlayerId, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const TRIGGER_SOURCE: &str = "TST-TRIGGERED-SACRIFICE-DERIVED-CONTROL-SOURCE";
const CONTROL_SOURCE: &str = "TST-TRIGGERED-SACRIFICE-DERIVED-CONTROL-AURA";
const STOLEN_CREATURE: &str = "TST-TRIGGERED-SACRIFICE-DERIVED-CONTROL-STOLEN";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["triggered-sacrifice-derived-control-red"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves the trigger into its choice");
}

#[test]
fn triggered_sacrifice_choice_accepts_a_creature_the_controller_stole() {
    let binding = TriggeredAbilityBinding {
        card_definition: TRIGGER_SOURCE,
        ability: TriggeredAbility {
            id: "upkeep-sacrifice",
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::SacrificeControllerCreature],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            creature(TRIGGER_SOURCE),
            creature(CONTROL_SOURCE),
            creature(STOLEN_CREATURE),
        ],
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("fixture initializes");
    game.put_on_battlefield(PlayerId(0), TRIGGER_SOURCE)
        .expect("upkeep trigger source begins on player zero battlefield");
    let control_source = game
        .put_on_battlefield(PlayerId(0), CONTROL_SOURCE)
        .expect("control source begins on player zero battlefield");
    let stolen = game
        .put_on_battlefield(PlayerId(1), STOLEN_CREATURE)
        .expect("creature begins under player one ownership and control");
    game.begin_game().expect("fixture begins at upkeep");
    game.clear_event_log();
    game.add_continuous_effect(
        control_source,
        stolen,
        ContinuousChange::ChangeController(PlayerId(0)),
        Duration::Permanent,
    )
    .expect("layer-two effect gives player zero control of the creature");
    assert_eq!(
        game.controller_of(stolen)
            .expect("stolen creature remains on battlefield"),
        PlayerId(0)
    );

    pass_pair(&mut game);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("resolving trigger opens a public sacrifice decision");
    assert!(
        decision.candidates.iter().any(|candidate| candidate.id == stolen),
        "the decision projects every creature the trigger controller currently controls"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![stolen]),
    )
    .expect("controller may sacrifice the stolen creature");

    eprintln!(
        "triggered_sacrifice_derived_control_events={:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.zone_of(stolen),
        Some(Zone::Graveyard),
        "a sacrificed stolen creature moves to its owner's graveyard"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedByEffect {
            source,
            player: PlayerId(0),
            permanent,
        } if *source != control_source && *permanent == stolen
    )));
    game.validate_invariants()
        .expect("derived-control trigger choice preserves state-machine invariants");
}
