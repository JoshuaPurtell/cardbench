//! Red discovery regression for Trophy Hunter's source-aware damage trigger.

use cardbench_magic_engine::{
    AbilityActivation, Color, CounterKind, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId,
    Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

const ABILITY: &str = "deal-one-damage-to-flying-creature";
const TRIGGER: &str = "damaged-creature-dies-add-plus-one-counter";

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn pay_activation(game: &mut Game) {
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on battlefield");
    game.begin_game().expect("fixture begins game");
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest supplies the colored activation mana");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("generic activation mana setup");
}

#[test]
fn trophy_hunter_damages_flyer_then_triggers_only_when_it_dies() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TROPHY-HUNTER")
        .expect("Trophy Hunter definition exists");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(2, [Color::Green]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(3)));
    assert!(definition.supported_rules.contains(&"activated-flying-targeted-one-damage"));
    assert!(definition.supported_rules.contains(&"source-dealt-damage-delayed-death-trigger"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));

    let activation = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id && binding.ability.id == ABILITY)
        .expect("Trophy Hunter activation binding exists");
    assert_eq!(activation.ability.mana_cost, ManaCost::with_colors(1, [Color::Green]));
    assert!(activation.ability.tap_cost);
    assert!(!activation.ability.sorcery_speed);
    assert_eq!(activation.ability.targets, [TargetRequirement::FlyingCreature]);
    assert_eq!(
        activation.ability.effects,
        [Effect::DealDamage {
            amount: 1,
            target: TargetRequirement::FlyingCreature,
        }]
    );
    let trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id && binding.ability.id == TRIGGER)
        .expect("delayed damage/death trigger binding exists");
    assert!(trigger.ability.targets.is_empty());
    assert_eq!(trigger.ability.effects, [Effect::AddPlusOneCounterToSource]);

    let mut game = game_with_rav_bindings();
    let hunter = game
        .put_on_battlefield(PlayerId(0), "RAV-TROPHY-HUNTER")
        .expect("Hunter begins on battlefield");
    let flyer = game
        .put_on_battlefield(PlayerId(1), "RAV-BIRDS-OF-PARADISE")
        .expect("one-toughness flyer begins on battlefield");
    pay_activation(&mut game);
    game.clear_event_log();
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: hunter,
            ability_id: ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(flyer)],
        },
    )
    .expect("Hunter deals damage through a stack ability");
    pass_pair(&mut game);

    assert_eq!(game.zone_of(flyer), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 }
            if *source == hunter && *permanent == flyer
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER, .. }
            if *source == hunter
    )));
    assert_eq!(
        game.object(hunter)
            .expect("Hunter remains on battlefield")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        None,
        "the trigger waits for its own priority window to resolve"
    );
    pass_pair(&mut game);
    assert_eq!(
        game.object(hunter)
            .expect("Hunter remains on battlefield")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    game.validate_invariants()
        .expect("source-aware Hunter trace preserves invariants");
}

#[test]
fn trophy_hunter_rejects_nonflying_target_before_mana_payment() {
    let mut game = game_with_rav_bindings();
    let hunter = game
        .put_on_battlefield(PlayerId(0), "RAV-TROPHY-HUNTER")
        .expect("Hunter begins on battlefield");
    let nonflyer = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("nonflying creature begins on battlefield");
    pay_activation(&mut game);
    game.clear_event_log();
    let mana_before = game
        .player(PlayerId(0))
        .expect("player exists")
        .mana_pool
        .clone();
    let result = game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: hunter,
            ability_id: ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(nonflyer)],
        },
    );
    assert!(result.is_err(), "a nonflying target is illegal");
    assert_eq!(game.player(PlayerId(0)).expect("player exists").mana_pool, mana_before);
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected Hunter activation preserves invariants");
}
