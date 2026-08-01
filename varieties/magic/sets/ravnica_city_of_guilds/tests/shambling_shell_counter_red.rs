//! Red discovery regression for Shambling Shell's source-sacrifice counter.

use cardbench_magic_engine::{
    AbilityActivation, Effect, Game, GameEvent, PlayerId, Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

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

#[test]
fn shambling_shell_requires_source_sacrifice_target_counter_activation() {
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-SHAMBLING-SHELL"
                && binding.ability.id == "shambling-shell-sacrifice-counter"
        })
        .expect("Shambling Shell counter binding exists");
    assert!(binding.ability.sacrifice_source);
    assert_eq!(binding.ability.targets, [TargetRequirement::Creature]);
    assert_eq!(binding.ability.effects, [Effect::AddPlusOneCounterToTarget]);

    let mut game = game_with_rav_bindings();
    let shell = game
        .put_on_battlefield(PlayerId(0), "RAV-SHAMBLING-SHELL")
        .expect("Shell begins on battlefield");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("target begins on battlefield");
    game.begin_game().expect("fixture begins game");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: shell,
            ability_id: "shambling-shell-sacrifice-counter",
            sacrifice_sources: vec![shell],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Shell ability enters the stack");
    assert_eq!(game.zone_of(shell), Some(Zone::Graveyard));
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
    println!("shambling_shell_event_log={:?}", game.canonical_event_log());
    assert_eq!(game.object(target).unwrap().counters.get("+1/+1"), Some(&1));
    assert_eq!(
        (
            game.characteristics(target).unwrap().power,
            game.characteristics(target).unwrap().toughness
        ),
        (Some(4), Some(4))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced { source, card, counter: "+1/+1", amount: 1 }
            if *source == shell && *card == target
    )));
    game.validate_invariants()
        .expect("Shell counter trace preserves invariants");
}

#[test]
fn shambling_shell_rejects_noncreature_target_before_sacrificing_source() {
    let mut game = game_with_rav_bindings();
    let shell = game
        .put_on_battlefield(PlayerId(0), "RAV-SHAMBLING-SHELL")
        .expect("Shell begins on battlefield");
    let land = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("land begins on battlefield");
    game.begin_game().expect("fixture begins game");
    game.clear_event_log();
    let result = game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: shell,
            ability_id: "shambling-shell-sacrifice-counter",
            sacrifice_sources: vec![shell],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(land)],
        },
    );
    assert!(result.is_err(), "a noncreature target is illegal");
    assert_eq!(game.zone_of(shell), Some(Zone::Battlefield));
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected Shell activation preserves invariants");
}
