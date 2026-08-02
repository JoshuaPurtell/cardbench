//! Red discovery contract for the generic "another creature leaves the
//! battlefield" trigger condition needed by Twilight Drover.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    Target, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

#[test]
fn engine_exposes_another_creature_leaves_battlefield_trigger_condition() {
    assert_eq!(
        TriggerCondition::AnotherCreatureLeavesBattlefield,
        TriggerCondition::AnotherCreatureLeavesBattlefield
    );
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: "LEAVE-OBSERVER",
            name: "Leave Observer",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["test-only"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: "LEAVE-EXIT",
            name: "Leave Exit",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["test-only"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: "LEAVE-BOUNCE",
            name: "Leave Bounce",
            set_code: "TST",
            mana_cost: ManaCost::with_colors(0, [Color::Blue]),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["test-only"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 1 }],
        },
        CardDefinition {
            id: "LEAVE-ISLAND",
            name: "Leave Island",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Blue]),
            card_types: BTreeSet::from([CardType::Land]),
            is_basic_land: false,
            supported_rules: &["test-only"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

#[test]
fn another_creature_leaves_trigger_observes_non_death_bounce() {
    let trigger = TriggeredAbilityBinding {
        card_definition: "LEAVE-OBSERVER",
        ability: TriggeredAbility {
            id: "another-leaves-counter",
            condition: TriggerCondition::AnotherCreatureLeavesBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AddPlusOneCounterToSource],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions(),
        2,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        [trigger],
    )
    .expect("synthetic leave trigger game builds");
    let observer = game
        .put_on_battlefield(PlayerId(0), "LEAVE-OBSERVER")
        .expect("observer enters");
    let exit = game
        .put_on_battlefield(PlayerId(0), "LEAVE-EXIT")
        .expect("exit permanent enters");
    let island = game
        .put_on_battlefield(PlayerId(0), "LEAVE-ISLAND")
        .expect("payment source enters");
    let bounce = game
        .add_card(PlayerId(0), "LEAVE-BOUNCE", Zone::Hand)
        .expect("bounce spell enters hand");
    game.grant_mana(PlayerId(0), Color::Blue, 1)
        .expect("pre-game payment setup");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: bounce,
            targets: vec![Target::Permanent(exit)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("bounce casts");
    game.pass_priority(PlayerId(0)).expect("bounce pass");
    game.pass_priority(PlayerId(1)).expect("bounce resolves");
    assert_eq!(game.zone_of(exit), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == observer && *ability == "another-leaves-counter"
    )));
    game.pass_priority(PlayerId(0)).expect("trigger pass");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");
    assert_eq!(
        game.object(observer)
            .expect("observer remains")
            .counters
            .get(&cardbench_magic_engine::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert_eq!(game.zone_of(island), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("leave trigger invariants");
}
