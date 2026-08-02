//! Regression coverage for Mausoleum Turnkey's conditional graveyard-return ETB.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_land_entry_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};

#[test]
fn mausoleum_turnkey_declares_its_conditional_graveyard_return_slice() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MAUSOLEUM-TURNKEY")
        .expect("Mausoleum Turnkey definition exists");
    assert!(
        definition
            .supported_rules
            .contains(&"enter-battlefield-conditional-graveyard-return-to-hand")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-MAUSOLEUM-TURNKEY"
            && binding.ability.id == "conditional-graveyard-return"
    }));
}

fn trigger_game() -> Game {
    Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .expect("RAV trigger fixture constructs")
}

fn cast_and_resolve(
    game: &mut Game,
    player: PlayerId,
    card: cardbench_magic_engine::ObjectId,
    targets: Vec<Target>,
) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell casts");
    game.pass_priority(player).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
}

#[test]
fn mausoleum_turnkey_stacks_and_resolves_when_another_creature_remains() {
    let mut game = trigger_game();
    let turnkey = game
        .add_card(PlayerId(0), "RAV-MAUSOLEUM-TURNKEY", Zone::Hand)
        .expect("Turnkey setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("target creature setup");
    let other = game
        .add_card(PlayerId(0), "RAV-BOROS-RECRUIT", Zone::Graveyard)
        .expect("other creature setup");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("setup mana");

    cast_and_resolve(&mut game, PlayerId(0), turnkey, vec![]);
    assert_eq!(game.stack.len(), 1, "conditional ETB is stacked");
    assert_eq!(game.stack[0].targets, vec![Target::Permanent(target)]);
    game.pass_priority(PlayerId(0))
        .expect("controller passes ETB");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves ETB");
    assert_eq!(game.zone_of(target), Some(Zone::Hand));
    assert_eq!(game.zone_of(other), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::AbilityResolved { source, ability, .. }
                if *source == turnkey && *ability == "conditional-graveyard-return"
        )
    }));
    assert_eq!(
        game.canonical_event_log(),
        vec![
            "SpellCast { player: PlayerId(0), card: ObjectId(1) }",
            "PriorityPassed { player: PlayerId(0) }",
            "PriorityPassed { player: PlayerId(1) }",
            "SpellResolved { card: ObjectId(1) }",
            "CardMoved { card: ObjectId(1), to: Battlefield }",
            "TriggeredAbilityStacked { controller: PlayerId(0), source: ObjectId(1), ability: \"conditional-graveyard-return\" }",
            "PriorityPassed { player: PlayerId(0) }",
            "PriorityPassed { player: PlayerId(1) }",
            "CardMoved { card: ObjectId(2), to: Hand }",
            "AbilityResolved { source: ObjectId(1), ability: \"conditional-graveyard-return\" }",
        ]
    );
    game.validate_invariants()
        .expect("valid conditional return");
}

#[test]
fn mausoleum_turnkey_does_not_stack_without_another_creature_card() {
    let mut game = trigger_game();
    let turnkey = game
        .add_card(PlayerId(0), "RAV-MAUSOLEUM-TURNKEY", Zone::Hand)
        .expect("Turnkey setup");
    let only = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("only creature setup");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("setup mana");

    cast_and_resolve(&mut game, PlayerId(0), turnkey, vec![]);
    assert!(
        game.stack.is_empty(),
        "intervening condition prevents stacking"
    );
    assert_eq!(game.zone_of(only), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("valid no-trigger boundary");
}
