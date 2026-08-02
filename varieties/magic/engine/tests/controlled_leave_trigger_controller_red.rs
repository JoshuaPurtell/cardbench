//! RED regression: a leave-the-battlefield trigger belongs to the source's
//! live controller, not its owner/base-controller field.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Duration, Effect, Game, GameEvent,
    ManaCost, PlayerId, Target, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
    Zone,
};

const OBSERVER: &str = "TST-CONTROLLED-LEAVE-OBSERVER";
const EXIT: &str = "TST-CONTROLLED-LEAVE-EXIT";
const CONTROL_SOURCE: &str = "TST-CONTROLLED-LEAVE-SOURCE";
const BOUNCE: &str = "TST-CONTROLLED-LEAVE-BOUNCE";

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let card_types: BTreeSet<_> = card_types.into_iter().collect();
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: card_types.clone(),
        is_basic_land: false,
        supported_rules: &["synthetic-controlled-leave-trigger-contract"],
        power: card_types.contains(&CardType::Creature).then_some(1),
        toughness: card_types.contains(&CardType::Creature).then_some(1),
        keywords: vec![],
        effects,
    }
}

#[test]
fn stolen_leave_trigger_stacks_for_its_live_controller() {
    let owner = PlayerId(0);
    let live_controller = PlayerId(1);
    let control_source_controller = PlayerId(2);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(OBSERVER, [CardType::Creature], vec![]),
            definition(EXIT, [CardType::Creature], vec![]),
            definition(CONTROL_SOURCE, [CardType::Artifact], vec![]),
            definition(
                BOUNCE,
                [CardType::Instant],
                vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 1 }],
            ),
        ],
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
    .expect("three-player control fixture initializes");
    let observer = game
        .put_on_battlefield(owner, OBSERVER)
        .expect("owner deploys observer");
    let exit = game
        .put_on_battlefield(owner, EXIT)
        .expect("owner deploys another creature");
    let control_source = game
        .put_on_battlefield(control_source_controller, CONTROL_SOURCE)
        .expect("third player deploys control source");
    let bounce = game
        .add_card(control_source_controller, BOUNCE, Zone::Hand)
        .expect("third player receives bounce spell");

    game.begin_game().expect("game starts");
    game.add_continuous_effect(
        control_source,
        observer,
        ContinuousChange::ChangeController(live_controller),
        Duration::Permanent,
    )
    .expect("observer changes control through layer two");
    assert_eq!(game.controller_of(observer), Ok(live_controller));

    for player in [owner, live_controller] {
        game.pass_priority(player)
            .expect("earlier player passes before bounce");
    }
    game.cast_spell(
        control_source_controller,
        CastRequest {
            card: bounce,
            targets: vec![Target::Permanent(exit)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("third player casts bounce");
    for player in [control_source_controller, owner, live_controller] {
        game.pass_priority(player)
            .expect("all players pass the bounce");
    }

    assert_eq!(game.zone_of(exit), Some(Zone::Hand));
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { controller, source, ability, .. }
                if *controller == live_controller && *source == observer && *ability == "another-creature-left"
        )),
        "the leave trigger must use its layer-two controller; events: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("the controlled trigger stays valid on the stack");
}
