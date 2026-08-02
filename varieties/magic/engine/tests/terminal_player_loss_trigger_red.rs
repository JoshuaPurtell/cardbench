//! RED regression: a terminal player-loss transition must not stack triggers
//! after the game has ended.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, GameEvent, ManaCost, PlayerId, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};

const OBSERVER: &str = "TST-TERMINAL-LEAVE-OBSERVER";
const DEPARTING_CREATURE: &str = "TST-TERMINAL-LEAVE-CREATURE";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["synthetic-terminal-loss-trigger-contract"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn terminal_owner_departure_discards_pending_leave_triggers_before_game_end() {
    let survivor = PlayerId(0);
    let departing_owner = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [creature(OBSERVER), creature(DEPARTING_CREATURE)],
        2,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        [TriggeredAbilityBinding {
            card_definition: OBSERVER,
            ability: TriggeredAbility {
                id: "another-creature-left",
                condition: TriggerCondition::AnotherCreatureLeavesBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::AddPlusOneCounterToSource],
            },
        }],
    )
    .expect("two-player terminal-loss fixture initializes");
    let observer = game
        .put_on_battlefield(survivor, OBSERVER)
        .expect("surviving observer enters");
    game.put_on_battlefield(departing_owner, DEPARTING_CREATURE)
        .expect("departing player has a creature");

    game.begin_game().expect("game starts");
    game.players[departing_owner.0].life = 0;
    game.check_state_based_actions()
        .expect("terminal player loss reaches its state boundary");

    assert!(game.is_game_over());
    assert!(
        game.stack.is_empty(),
        "a trigger must not remain on stack once the game has ended; events: {:?}",
        game.canonical_event_log()
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == observer && *ability == "another-creature-left"
        )),
        "terminal loss discards triggered-but-unstacked events; events: {:?}",
        game.canonical_event_log()
    );
    assert!(matches!(
        game.event_log.last(),
        Some(GameEvent::GameEnded {
            winner: Some(player)
        }) if *player == survivor
    ));
    game.validate_invariants()
        .expect("the terminal state has no actionable stack state");
}
