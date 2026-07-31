//! Red regression for Selesnya Evangel's compound activated-ability cost.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, ManaCost, PlayerId,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn selesnya_evangel_has_its_exact_cost_and_compound_tap_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELESNYA-EVANGEL")
        .expect("Selesnya Evangel definition exists");
    assert_eq!(definition.name, "Selesnya Evangel");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::Green, Color::White])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(2)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"activated-token-creation-with-creature-tap-cost")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-SELESNYA-EVANGEL"
            && binding.ability.id == "create-saproling"
    }));
}

#[test]
fn selesnya_evangel_pays_mana_and_two_distinct_creature_taps_before_stacking_token_creation() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let evangel = game
        .put_on_battlefield(PlayerId(0), "RAV-SELESNYA-EVANGEL")
        .expect("Selesnya Evangel enters");
    let companion = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("another controlled creature enters");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest enters");
    for permanent in [evangel, companion, forest] {
        game.set_entered_turn_for_setup(permanent, 0)
            .expect("fixture permanent predates measured turn");
    }
    game.begin_game().expect("game starts");
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest pays Evangel's generic cost");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: evangel,
            ability_id: "create-saproling",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("compound Evangel activation succeeds");
    assert!(game.object(evangel).expect("Evangel remains").tapped);
    assert!(game.object(companion).expect("companion remains").tapped);
    assert_eq!(game.stack.len(), 1, "ability must use the stack");

    game.pass_priority(PlayerId(0))
        .expect("controller passes token ability");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes token ability");

    println!("Selesnya Evangel trace: {:?}", game.canonical_event_log());
    assert_eq!(
        game.player(PlayerId(0))
            .expect("controller exists")
            .battlefield
            .len(),
        4
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("compound tap activation preserves invariants");
}
