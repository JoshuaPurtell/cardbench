//! RED regression: a trigger created during Cleanup must keep that Cleanup
//! step open for the required priority window, then cause another Cleanup.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId,
    Step, TokenSpec, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const OBSERVER: &str = "TST-CLEANUP-OBSERVER";
const CREATE_AND_PUMP: &str = "TST-CLEANUP-CREATE-AND-PUMP";

fn creature_definition(id: &'static str, power: i16, toughness: i16) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["synthetic-cleanup-trigger-contract"],
        power: Some(power),
        toughness: Some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

fn create_and_pump_definition() -> CardDefinition {
    CardDefinition {
        id: CREATE_AND_PUMP,
        name: CREATE_AND_PUMP,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["synthetic-cleanup-trigger-contract"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::CreateToken {
                token: TokenSpec {
                    name: "doomed token",
                    is_legendary: false,
                    colors: BTreeSet::new(),
                    card_types: BTreeSet::from([CardType::Creature]),
                    creature_subtypes: BTreeSet::new(),
                    keywords: vec![],
                    power: 0,
                    toughness: 0,
                },
                count: 1,
            },
            Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
        ],
    }
}

fn pass_round(game: &mut Game) {
    if game.step == Step::DeclareAttackers {
        game.declare_attackers(game.active_player, &[])
            .expect("empty attack declaration");
    }
    for _ in 0..2 {
        game.pass_priority(game.priority)
            .expect("current priority holder passes");
    }
}

fn cleanup_fixture() -> (Game, ObjectId, PlayerId, PlayerId) {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let trigger = TriggeredAbilityBinding {
        card_definition: OBSERVER,
        ability: TriggeredAbility {
            id: "another-creature-died-at-cleanup",
            condition: TriggerCondition::AnotherCreatureDies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AddPlusOneCounterToSource],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            creature_definition(OBSERVER, 1, 1),
            create_and_pump_definition(),
        ],
        2,
        [],
        [],
        [],
        [],
        [trigger],
    )
    .expect("fixture game builds");
    let observer = game
        .add_card(first, OBSERVER, Zone::Battlefield)
        .expect("observer setup");
    let create_and_pump = game
        .add_card(first, CREATE_AND_PUMP, Zone::Hand)
        .expect("token spell setup");
    game.begin_game().expect("game begins");
    game.cast_spell(
        first,
        CastRequest {
            card: create_and_pump,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("token-and-pump spell casts");
    for player in [first, second] {
        game.pass_priority(player).expect("token spell resolves");
    }
    (game, observer, first, second)
}

#[test]
fn cleanup_sba_trigger_opens_priority_before_the_next_turn() {
    let (mut game, observer, first, second) = cleanup_fixture();

    for _ in 0..8 {
        pass_round(&mut game);
        if game.step == Step::Cleanup || game.turn != 1 {
            break;
        }
    }

    eprintln!(
        "cleanup-trigger red trace: {:?}",
        game.canonical_event_log()
    );
    assert!(
        game.event_log
            .iter()
            .any(|event| matches!(event, GameEvent::TokenCeasedToExist { .. }))
    );
    assert_eq!(
        game.step,
        Step::Cleanup,
        "a cleanup-created trigger must create the exceptional Cleanup priority window, not advance to the next turn; trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.turn, 1);
    assert_eq!(game.priority, first);
    assert!(game.stack.iter().any(|item| {
        item.card == observer && item.ability_id == Some("another-creature-died-at-cleanup")
    }));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == observer && *ability == "another-creature-died-at-cleanup"
    )));
    game.validate_invariants()
        .expect("cleanup-trigger priority state is auditable");

    // The first player, not the next turn's player, has the opportunity to
    // act before the pending trigger resolves.
    assert_eq!(game.next_policy_player(), first);

    for player in [first, second] {
        game.pass_priority(player)
            .expect("the cleanup trigger resolves after both players pass");
    }
    assert!(game.stack.is_empty());
    assert_eq!(game.step, Step::Cleanup);
    assert_eq!(game.priority, first);

    for player in [first, second] {
        game.pass_priority(player)
            .expect("the exceptional cleanup window closes");
    }
    assert_eq!(game.turn, 2);
    assert_eq!(game.step, Step::Upkeep);
    assert_eq!(game.priority, second);
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::StepBegan {
                    step: Step::Cleanup,
                    ..
                }
            ))
            .count(),
        2,
        "the trigger window must be followed by one repeated Cleanup"
    );
    game.validate_invariants()
        .expect("repeated cleanup returns to the ordinary turn machine");
}
