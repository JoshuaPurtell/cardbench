//! Red regression: rejected public effect installation must be transactionally inert.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Duration, Game, PlayerId,
};

const BODY: &str = "TEST-TERMINAL-EFFECT-BODY";

fn body_definition() -> CardDefinition {
    CardDefinition {
        id: BODY,
        name: BODY,
        set_code: "TST",
        mana_cost: cardbench_magic_engine::ManaCost::new(0),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["test-base-characteristics"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn rejected_terminal_effect_installation_is_event_and_state_atomic() {
    let survivor = PlayerId(0);
    let eliminated = PlayerId(1);
    let mut game = Game::new(vec![body_definition()], 2).expect("two-player fixture initializes");
    let source = game
        .put_on_battlefield(survivor, BODY)
        .expect("source enters battlefield");
    let target = game
        .put_on_battlefield(survivor, BODY)
        .expect("target enters battlefield");
    game.set_fixture_player_life(eliminated, 0)
        .expect("fixture marks player at zero life");
    game.check_state_based_actions()
        .expect("fixture reaches terminal loss through the public SBA seam");
    let effects_before = game.continuous_effects.clone();
    let events_before = game.event_log.clone();

    let result = game.add_continuous_effect(
        source,
        target,
        ContinuousChange::AddColor(Color::Blue),
        Duration::Permanent,
    );

    assert!(result.is_err(), "terminal installation must reject");
    assert_eq!(
        game.continuous_effects,
        effects_before,
        "a rejected terminal effect installation mutated continuous effects; result={result:?}; events before={events_before:?}; events after={:?}; invariant={:?}",
        game.event_log,
        game.validate_invariants(),
    );
    assert_eq!(
        game.event_log, events_before,
        "a rejected terminal effect installation appended a lifecycle event"
    );
    game.validate_invariants()
        .expect("rejection must preserve the terminal lifecycle invariant");
}
