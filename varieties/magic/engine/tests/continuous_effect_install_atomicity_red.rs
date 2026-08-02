//! Red regression: a rejected public continuous-effect installation must not
//! leave its effect, object mutation, or canonical receipt behind.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, ContinuousChange, Duration, Game, ManaCost, PlayerId,
};

const SOURCE: &str = "CONTINUOUS-ATOMIC-SOURCE";
const TARGET: &str = "CONTINUOUS-ATOMIC-TARGET";

fn definition(id: &'static str, card_type: CardType, power: Option<i16>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["continuous-effect-atomic-installation"],
        power,
        toughness: power,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn rejected_negative_damage_shield_installation_is_atomic() {
    let controller = PlayerId(0);
    let mut game = Game::new(
        [
            definition(SOURCE, CardType::Artifact, None),
            definition(TARGET, CardType::Creature, Some(2)),
        ],
        2,
    )
    .expect("fixture initializes");
    let source = game
        .put_on_battlefield(controller, SOURCE)
        .expect("source setup");
    let target = game
        .put_on_battlefield(controller, TARGET)
        .expect("target setup");
    game.begin_game().expect("game begins");
    let before_events = game.canonical_event_log();

    let result = game.add_continuous_effect(
        source,
        target,
        ContinuousChange::AddDamageShield(-1),
        Duration::EndOfTurn(game.turn),
    );
    eprintln!(
        "negative-shield install: {result:?}; shield={:?}; effects={:?}; events={:?}",
        game.object(target).map(|object| object.damage_shield),
        game.continuous_effects,
        game.canonical_event_log(),
    );
    assert!(result.is_err(), "a nonpositive damage shield is illegal");
    assert_eq!(
        game.object(target)
            .expect("target remains live")
            .damage_shield,
        0,
        "a rejected installation must not mutate the target's shield state",
    );
    assert!(
        game.continuous_effects.is_empty(),
        "a rejected installation must not leave a live effect",
    );
    assert_eq!(
        game.canonical_event_log(),
        before_events,
        "a rejected installation must not write a lifecycle receipt",
    );
    game.validate_invariants()
        .expect("the rejected installation leaves an invariant-valid state");
}
