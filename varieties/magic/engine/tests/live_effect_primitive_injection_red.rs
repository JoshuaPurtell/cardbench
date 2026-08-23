//! Red regression: public effect primitives cannot become live out-of-turn moves.
//!
//! The fixture helpers are useful before a game begins, but a public transition
//! that changes a started game needs an authorized policy/priority provenance.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Duration, Game, GameEvent, ManaCost,
    ObjectId, PlayerId,
};

const SOURCE: &str = "TST-LIVE-EFFECT-PRIMITIVE-SOURCE";
const TARGET: &str = "TST-LIVE-EFFECT-PRIMITIVE-TARGET";

fn creature(id: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([color]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["live-effect-primitive-injection-red"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn game_with_opponent_priority() -> (Game, ObjectId, ObjectId) {
    let mut game = Game::new(
        [creature(SOURCE, Color::Blue), creature(TARGET, Color::Red)],
        2,
    )
    .expect("fixture initializes");
    let source = game
        .put_on_battlefield(PlayerId(0), SOURCE)
        .expect("source setup");
    let target = game
        .put_on_battlefield(PlayerId(0), TARGET)
        .expect("target setup");
    game.begin_game().expect("game begins");
    game.pass_priority(PlayerId(0))
        .expect("player zero passes priority");
    assert_eq!(game.priority, PlayerId(1));
    (game, source, target)
}

#[test]
fn copy_permanent_rejects_a_live_call_from_a_player_without_priority() {
    let (mut game, source, target) = game_with_opponent_priority();
    let before_events = game.canonical_event_log();

    let result = game.copy_permanent(target, source);
    eprintln!(
        "copy result={result:?}; priority={:?}; events={:?}",
        game.priority,
        game.canonical_event_log()
    );

    assert!(
        result.is_err(),
        "a direct live copy must not bypass the opponent's priority and policy move"
    );
    assert_eq!(game.canonical_event_log(), before_events);
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::PermanentCopied { .. }))
    );
}

#[test]
fn add_continuous_effect_rejects_a_live_call_from_a_player_without_priority() {
    let (mut game, source, target) = game_with_opponent_priority();
    let before_events = game.canonical_event_log();

    let result = game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ModifyPowerToughness {
            power: 1,
            toughness: 1,
        },
        Duration::EndOfTurn(game.turn),
    );
    eprintln!(
        "continuous result={result:?}; priority={:?}; events={:?}",
        game.priority,
        game.canonical_event_log()
    );

    assert!(
        result.is_err(),
        "a direct live continuous effect must not bypass the opponent's priority and policy move"
    );
    assert_eq!(game.canonical_event_log(), before_events);
    assert!(game.continuous_effects.is_empty());
}
