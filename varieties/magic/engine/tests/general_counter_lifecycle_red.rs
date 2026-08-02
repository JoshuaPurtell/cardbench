//! Red discovery regression for the bounded named-counter substrate.
//!
//! The pre-generalization engine represents counters as strings but silently
//! refuses to place its only supported counter on a noncreature permanent.
//! Counters are battlefield-object state, not creature-only state: a charge
//! or named counter on an artifact must be representable without inventing a
//! card-specific exception.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    Effect, Game, GameEvent, ManaCost, PlayerId,
};

const COUNTER_ENGINE: &str = "TST-COUNTER-ENGINE";

fn counter_engine() -> CardDefinition {
    CardDefinition {
        id: COUNTER_ENGINE,
        name: "Counter engine fixture",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["general-counter-lifecycle-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player resolves");
}

#[test]
fn a_battlefield_artifact_can_receive_persistent_counter_state() {
    let mut game = Game::new_with_all_bindings(
        [counter_engine()],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: COUNTER_ENGINE,
            ability: ActivatedAbility {
                id: "add-counter",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::AddPlusOneCounterToSource],
            },
        }],
    )
    .expect("fixture game constructs");
    let artifact = game
        .put_on_battlefield(PlayerId(0), COUNTER_ENGINE)
        .expect("artifact begins on battlefield");
    game.begin_game().expect("fixture game begins");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: artifact,
            ability_id: "add-counter",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("ability enters the stack");
    pass_pair(&mut game);

    eprintln!(
        "general_counter_red state={:?}; events={:?}",
        game.object(artifact).expect("artifact remains live"),
        game.canonical_event_log(),
    );
    assert_eq!(
        game.object(artifact)
            .expect("artifact remains live")
            .counters
            .get("+1/+1"),
        Some(&1),
        "a battlefield permanent counter must not depend on creature type",
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced { card, counter: "+1/+1", amount: 1, .. }
            if *card == artifact
    )));
    game.validate_invariants()
        .expect("counter lifecycle leaves a valid state");
}
