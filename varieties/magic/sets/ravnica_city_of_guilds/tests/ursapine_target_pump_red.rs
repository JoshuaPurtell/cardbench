//! Red discovery regression for the unported Ursapine activation.

use cardbench_magic_engine::{
    AbilityActivation, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn ursapine_requires_its_targeted_green_pump_binding_and_stack_trace() {
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-URSAPINE"
                && binding.ability.id == "ursapine-target-pump"
        })
        .expect("Ursapine targeted-pump binding exists");
    assert_eq!(
        binding.ability.mana_cost,
        ManaCost::with_colors(0, [Color::Green])
    );
    assert_eq!(binding.ability.targets, [TargetRequirement::Creature]);
    assert_eq!(
        binding.ability.effects,
        [Effect::ModifyTargetPtUntilEndOfTurn {
            power: 1,
            toughness: 1,
        }]
    );

    let mut game = game_with_rav_bindings();
    let ursapine = game
        .put_on_battlefield(PlayerId(0), "RAV-URSAPINE")
        .expect("Ursapine begins on the battlefield");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("Watchwolf begins on the battlefield");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on the battlefield");
    game.begin_game().expect("fixture begins game");
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest pays activation");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: ursapine,
            ability_id: "ursapine-target-pump",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Ursapine ability enters the stack");
    resolve_top(&mut game);

    println!("ursapine_event_log={:?}", game.canonical_event_log());
    assert_eq!(
        (
            game.characteristics(target).unwrap().power,
            game.characteristics(target).unwrap().toughness
        ),
        (Some(4), Some(4))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityManaPaid { source, ability, mana_cost, .. }
            if *source == ursapine
                && *ability == "ursapine-target-pump"
                && *mana_cost == ManaCost::with_colors(0, [Color::Green])
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target: affected, .. }
            if *source == ursapine && *affected == target
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == ursapine && *ability == "ursapine-target-pump"
    )));
    game.validate_invariants()
        .expect("Ursapine trace preserves invariants");
}
