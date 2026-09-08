//! Red discovery contract for Halcyon Glaze's creature-spell animation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn rav_game() -> Game {
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

fn advance_to_main(game: &mut Game) {
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first player passes");
        let second = game.priority;
        game.pass_priority(second).expect("second player passes");
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

#[test]
fn halcyon_glaze_requires_creature_spell_self_animation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HALCYON-GLAZE")
        .expect("Halcyon Glaze definition exists");

    assert_eq!(definition.name, "Halcyon Glaze");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue, Color::Blue])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Halcyon Glaze cannot be complete while its creature-spell animation is absent"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"creature-spell-triggered-self-animation"),
        "Halcyon Glaze must expose its temporary self-animation rule"
    );
}

#[test]
fn halcyon_glaze_uses_the_stack_for_its_layered_creature_spell_animation() {
    let mut game = rav_game();
    let glaze = game
        .put_on_battlefield(PlayerId(0), "RAV-HALCYON-GLAZE")
        .expect("Halcyon Glaze setup");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest setup");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains setup");
    let watchwolf = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("Watchwolf is in hand");
    game.begin_game().expect("game begins");
    advance_to_main(&mut game);
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest supplies green mana");
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("Plains supplies white mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Watchwolf casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the trigger");
    game.pass_priority(PlayerId(1))
        .expect("Halcyon Glaze trigger resolves");
    let animated = game.characteristics(glaze).expect("Glaze is live");
    assert!(animated.card_types.contains(&CardType::Enchantment));
    assert!(animated.card_types.contains(&CardType::Creature));
    assert_eq!((animated.power, animated.toughness), (Some(4), Some(4)));
    assert!(animated.keywords.contains(&Keyword::Flying));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { ability, source, .. }
            if *ability == "creature-spell-animate-source-until-end-of-turn" && *source == glaze
    )));
    eprintln!("Halcyon Glaze trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Halcyon Glaze animation preserves invariants");
}
