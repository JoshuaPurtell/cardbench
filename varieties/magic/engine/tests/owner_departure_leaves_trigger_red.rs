//! RED regression: CR 800.4a owner departure is still a battlefield-leave
//! event for surviving permanents that observe another creature leaving.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, Game, GameEvent, ManaCost, PlayerId, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};

const OBSERVER: &str = "TST-OWNER-DEPARTURE-LEAVE-OBSERVER";
const DEPARTING_CREATURE: &str = "TST-OWNER-DEPARTURE-LEAVE-CREATURE";

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
        supported_rules: &["synthetic-owner-departure-leave-contract"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn surviving_observer_triggers_when_another_creatures_owner_leaves_the_game() {
    let observer_controller = PlayerId(0);
    let departing_owner = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [creature(OBSERVER), creature(DEPARTING_CREATURE)],
        3,
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
    .expect("three-player leave-trigger fixture initializes");
    let observer = game
        .put_on_battlefield(observer_controller, OBSERVER)
        .expect("surviving observer enters");
    let departing_creature = game
        .put_on_battlefield(departing_owner, DEPARTING_CREATURE)
        .expect("departing owner controls a creature");

    game.begin_game().expect("game starts");
    game.players[departing_owner.0].life = 0;
    game.check_state_based_actions()
        .expect("owner departure reaches the SBA fixed point");

    assert!(game.object(departing_creature).is_err());
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == observer && *ability == "another-creature-left"
        )),
        "the surviving observer must see the owner-departure battlefield event; events: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("owner departure and its queued trigger remain valid");
}
