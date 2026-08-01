//! Red discovery regression for Elvish Skysweeper's flying-only destruction.

use cardbench_magic_engine::{
    AbilityActivation, Color, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId, Target,
    TargetRequirement, Zone,
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

fn five_forests_for_setup(game: &mut Game) -> Vec<ObjectId> {
    let mut forests = Vec::new();
    for _ in 0..5 {
        forests.push(
            game.put_on_battlefield(PlayerId(0), "RAV-FOREST")
                .expect("Forest begins on battlefield"),
        );
    }
    forests
}

fn pay_five_green(game: &mut Game, forests: &[ObjectId]) {
    for forest in forests {
        game.activate_mana_ability(PlayerId(0), *forest, Color::Green)
            .expect("Forest provides green mana");
    }
}

#[test]
fn skysweeper_requires_flying_target_sacrifice_activation() {
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-ELVISH-SKYSWEEPER"
                && binding.ability.id == "sacrifice-creature-destroy-flying"
        })
        .expect("Elvish Skysweeper flying-destruction binding exists");
    assert_eq!(
        binding.ability.mana_cost,
        ManaCost::with_colors(4, [Color::Green])
    );
    assert_eq!(binding.ability.sacrifice_creatures, 1);
    assert_eq!(binding.ability.targets, [TargetRequirement::FlyingCreature]);
    assert_eq!(
        binding.ability.effects,
        [Effect::DestroyTargetFlyingCreature]
    );

    let mut game = game_with_rav_bindings();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-ELVISH-SKYSWEEPER")
        .expect("Skysweeper begins on battlefield");
    let sacrifice = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("sacrifice creature begins on battlefield");
    let flyer = game
        .put_on_battlefield(PlayerId(1), "RAV-BIRDS-OF-PARADISE")
        .expect("flying target begins on battlefield");
    let forests = five_forests_for_setup(&mut game);
    game.begin_game().expect("fixture begins game");
    pay_five_green(&mut game, &forests);
    game.clear_event_log();
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "sacrifice-creature-destroy-flying",
            sacrifice_sources: vec![sacrifice],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(flyer)],
        },
    )
    .expect("Skysweeper ability enters the stack");
    assert_eq!(game.zone_of(sacrifice), Some(Zone::Graveyard));
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
    println!(
        "elvish_skysweeper_event_log={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(flyer), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source: event_source, card }
            if *event_source == source && *card == flyer
    )));
    game.validate_invariants()
        .expect("Skysweeper trace preserves invariants");
}

#[test]
fn skysweeper_rejects_nonflying_target_before_payment_or_sacrifice() {
    let mut game = game_with_rav_bindings();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-ELVISH-SKYSWEEPER")
        .expect("Skysweeper begins on battlefield");
    let sacrifice = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("sacrifice creature begins on battlefield");
    let nonflyer = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("nonflying target begins on battlefield");
    let forests = five_forests_for_setup(&mut game);
    game.begin_game().expect("fixture begins game");
    pay_five_green(&mut game, &forests);
    game.clear_event_log();
    let mana_before = game
        .player(PlayerId(0))
        .expect("player exists")
        .mana_pool
        .clone();
    let result = game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "sacrifice-creature-destroy-flying",
            sacrifice_sources: vec![sacrifice],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(nonflyer)],
        },
    );
    assert!(result.is_err(), "a nonflying target is illegal");
    assert_eq!(game.zone_of(sacrifice), Some(Zone::Battlefield));
    assert_eq!(game.player(PlayerId(0)).unwrap().mana_pool, mana_before);
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected Skysweeper activation preserves invariants");
}
