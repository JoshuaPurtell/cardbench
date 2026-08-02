//! Red regression: SBAs apply before the first priority window.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Game, ManaCost, PlayerId, Zone};

const ZERO_TOUGHNESS: &str = "BEGIN-GAME-SBA-ZERO-TOUGHNESS";

fn zero_toughness_creature() -> CardDefinition {
    CardDefinition {
        id: ZERO_TOUGHNESS,
        name: ZERO_TOUGHNESS,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["begin-game-sba-boundary-probe"],
        power: Some(1),
        toughness: Some(0),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn game_start_applies_zero_toughness_sba_before_upkeep_priority() {
    let player = PlayerId(0);
    let mut game = Game::new([zero_toughness_creature()], 2).expect("fixture game initializes");
    let creature = game
        .put_on_battlefield(player, ZERO_TOUGHNESS)
        .expect("zero-toughness creature begins on the battlefield");

    game.begin_game()
        .expect("game start reaches first priority");
    eprintln!(
        "begin-game SBA boundary: zone={:?}; step={:?}; priority={:?}; events={:?}",
        game.zone_of(creature),
        game.step,
        game.priority,
        game.canonical_event_log(),
    );

    assert_eq!(
        game.zone_of(creature),
        Some(Zone::Graveyard),
        "a zero-toughness creature must leave before the first player receives priority"
    );
    assert!(
        game.canonical_event_log().iter().any(|event| {
            event.contains("StateBasedAction")
                && event.contains("creature has toughness zero or less")
        }),
        "the automatic SBA is auditable before first-upkeep priority"
    );
    assert_eq!(
        game.canonical_event_log(),
        [
            format!("StepBegan {{ turn: 1, active_player: {player:?}, step: Untap }}"),
            format!(
                "StateBasedAction {{ card: {creature:?}, reason: \"creature has toughness zero or less\" }}"
            ),
            format!("CardMoved {{ card: {creature:?}, to: Graveyard }}"),
            format!("ObjectIncarnationAdvanced {{ object: {creature:?}, incarnation: 2 }}"),
            format!("StepBegan {{ turn: 1, active_player: {player:?}, step: Upkeep }}"),
        ],
        "the turn boundary records Untap, stabilizes SBAs, then opens Upkeep priority"
    );
    game.validate_invariants()
        .expect("the first priority state is structurally valid");
}
